use std::borrow::Cow;
use std::marker::PhantomData;

use bevy::ecs::system::{EntityCommands, SystemParam, SystemState};
use bevy::prelude::*;
use bevy_egui::{
    egui::{self, Ui},
    EguiContexts,
};
use bevy_mod_picking::backend::{HitData, PointerHits};
use bevy_mod_picking::prelude::*;
use itertools::Itertools;
use std::fmt::Debug;

use crate::cursor::EntityCommandsStartDragExt;
use crate::widget::{WidgetId, WidgetSystem};

/// The alpha (out of 255) of an enabled waymark spawner widget.
const SPAWNER_ALPHA: u8 = 230;
/// The alpha (out of 255) of a disabled waymark spawner widget.
const SPAWNER_DISABLED_ALPHA: u8 = 25;

/// An entity that can be spawned.
pub trait Spawnable: Component + Reflect + TypePath + Clone + PartialEq + Debug {
    const UNIQUE: bool;

    fn spawner_name(&self) -> Cow<'static, str>;
    fn texture_handle(&self, asset_server: &AssetServer) -> Handle<Image>;
    fn insert(&self, entity: &mut EntityCommands);
}

/// An entity that can be clicked & dragged to spawn a waymark.
///
/// Rendered using egui, not the normal logic.
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(from_reflect = false)]
pub struct Spawner<Target: Spawnable> {
    pub target: Target,
    #[reflect(ignore)]
    pub texture_id: egui::TextureId,
    pub enabled: bool,
}

impl<T: Spawnable> Spawner<T> {
    // TODO: TEST TEST TEST
    pub fn update_enabled_state(mut q: Query<&mut Spawner<T>>, target_q: Query<&T>) {
        for mut spawner in &mut q {
            spawner.enabled = !target_q.iter().contains(&spawner.target);
        }
    }

    /// Handle a drag event, spawning a new [Waymark] in place of the current entity if
    /// the [Spawner] is enabled.
    ///
    /// Technically what it actually does is, to preserve continuity of the drag event,
    /// replaces this entity with the new waymark, and spawns a new [Spawner] in its place.
    ///
    /// Panics if there is more than one camera.
    pub fn drag_start(
        ev: Listener<Pointer<DragStart>>,
        spawner_q: Query<&Spawner<T>>,
        camera_q: Query<(&Camera, &GlobalTransform)>,
        mut commands: Commands,
    ) {
        let id = ev.listener();
        debug!("starting drag on spawner {id:?}");
        let Ok(spawner) = spawner_q.get(id) else {
            debug!("but it doesn't exist");
            return;
        };
        if !spawner.enabled {
            debug!("but it was disabled");
            return;
        }

        commands.spawn(SpawnerBundle {
            name: Name::new(format!("Spawner for {}", spawner.target.spawner_name())),
            spawner: spawner.clone(),
            pickable: default(),
            drag_start: On::<Pointer<DragStart>>::run(Self::drag_start),
        });

        let mut entity = commands.entity(id);
        entity.remove::<SpawnerBundle<T>>();

        let (camera, camera_transform) = camera_q.single();
        let hit_position = ev.hit.position.unwrap().truncate();
        let translation = camera
            .viewport_to_world_2d(camera_transform, hit_position)
            .unwrap()
            .extend(0.0);
        debug!(
            "spawner spawning waymark {:?} at {translation} (from hit position: {hit_position})",
            spawner.target,
        );

        spawner.target.insert(&mut entity);
        entity.insert(Transform::from_translation(translation));
        // Forward to the general dragging implementation.
        entity.start_drag();
    }
}

#[derive(SystemParam)]

pub struct SpawnerWidget<'w, 's, Target: Spawnable> {
    spawner_q: Query<'w, 's, (Entity, &'static Spawner<Target>)>,
    target_q: Query<'w, 's, &'static Target>,
    pointer_ev: EventWriter<'w, PointerHits>,
}

impl<T: Spawnable> WidgetSystem for SpawnerWidget<'_, '_, T> {
    type In = (T, Vec2);
    type Out = egui::Response;

    fn run_with(
        world: &mut World,
        state: &mut SystemState<Self>,
        ui: &mut Ui,
        _id: WidgetId,
        (target, size): (T, Vec2),
    ) -> Self::Out {
        let mut state = state.get_mut(world);
        let (id, spawner) = state
            .spawner_q
            .iter()
            .filter(|(_, spawner)| spawner.target == target)
            .exactly_one()
            .unwrap_or_else(|e| panic!("Tried to run a spawner that doesn't exist uniquely: {e}"));
        let resp = ui.add(
            egui::Image::new((spawner.texture_id, egui::Vec2::new(size.x, size.y)))
                .tint(egui::Color32::from_white_alpha(if spawner.enabled {
                    SPAWNER_ALPHA
                } else {
                    SPAWNER_DISABLED_ALPHA
                }))
                .sense(egui::Sense::drag()),
        );

        if resp.hovered() {
            let egui::Pos2 { x, y } = resp.hover_pos().unwrap();
            state.pointer_ev.send(PointerHits::new(
                PointerId::Mouse,
                vec![(id, HitData::new(id, 0.0, Some(Vec3::new(x, y, 0.0)), None))],
                // egui is at depth 1_000_000, we need to be in front of that.
                1_000_001.0,
            ));
        }

        resp
    }
}

/// Bundle of components for a [Spawner].
#[derive(Bundle)]
pub struct SpawnerBundle<T: Spawnable> {
    pub name: Name,
    pub spawner: Spawner<T>,
    pub pickable: PickableBundle,
    pub drag_start: On<Pointer<DragStart>>,
}

impl<T: Spawnable> SpawnerBundle<T> {
    pub fn new(entity: T, asset_server: &AssetServer, contexts: &mut EguiContexts) -> Self {
        let texture = entity.texture_handle(asset_server);
        Self {
            name: Name::new(format!("Spawner for {}", entity.spawner_name())),
            spawner: Spawner {
                target: entity,
                texture_id: contexts.add_image(texture),
                enabled: true,
            },
            pickable: default(),
            drag_start: On::<Pointer<DragStart>>::run(Spawner::<T>::drag_start),
        }
    }
}

/// Plugin for spawner support
#[derive(Copy, Clone, Debug)]
pub struct SpawnerPlugin<Target> {
    _phantom: PhantomData<Target>,
}

impl<T> Default for SpawnerPlugin<T> {
    fn default() -> Self {
        Self {
            _phantom: default(),
        }
    }
}

impl<T: Spawnable> Plugin for SpawnerPlugin<T> {
    fn build(&self, app: &mut App) {
        app.register_type::<Spawner<T>>();
        if <T as Spawnable>::UNIQUE {
            app.add_systems(PostUpdate, Spawner::<T>::update_enabled_state);
        }
    }
}

// TODO: Put this somewhere better lol.
fn log_debug<E: std::fmt::Debug + Event>(mut events: EventReader<E>) {
    for ev in events.read() {
        debug!("{ev:?}");
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::spawner::SpawnerBundle;
    use crate::waymark::Waymark;
    use crate::widget::egui_context;
    use crate::{testing::*, widget};

    use bevy::app::ScheduleRunnerPlugin;
    use bevy::render::settings::{RenderCreation, WgpuSettings};
    use bevy::render::RenderPlugin;
    use bevy::window::PrimaryWindow;
    use bevy::winit::WinitPlugin;
    use bevy_egui::EguiPlugin;
    use bevy_egui::{egui, EguiContexts};

    use bevy_mod_picking::DefaultPickingPlugins;
    use bevy_picking_core::events::{Drag, DragEnd, DragStart, Pointer};
    use float_eq::assert_float_eq;

    #[derive(Default, Resource)]
    struct TestWinPos(egui::Pos2);

    const SPAWNER_SIZE: f32 = 40.0;

    fn draw_test_win<T: Spawnable>(world: &mut World) {
        let ctx = egui_context(world);
        let pos = world.resource::<TestWinPos>().0;
        egui::Area::new("test").fixed_pos(pos).show(&ctx, |ui| {
            widget::show_with::<SpawnerWidget<Waymark>>(
                world,
                ui,
                WidgetId::new("spawner"),
                (Waymark::A, Vec2::splat(SPAWNER_SIZE)),
            );
        });
    }

    fn spawn_test_entities(
        mut commands: Commands,
        asset_server: Res<AssetServer>,
        mut contexts: EguiContexts,
    ) {
        commands.spawn(SpawnerBundle::new(Waymark::A, &asset_server, &mut contexts));
        commands.spawn(DragSurfaceBundle::new(Rect::from_center_half_size(
            Vec2::ZERO,
            Vec2::splat(100.0),
        )));
    }

    // returns the primary window ID and the app itself
    fn test_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(WgpuSettings {
                        backends: None,
                        ..default()
                    }),
                })
                .disable::<WinitPlugin>(),
        )
        // Allow for controlled looping & exit.
        .add_plugins(ScheduleRunnerPlugin {
            run_mode: bevy::app::RunMode::Loop { wait: None },
        })
        .add_plugins(EguiPlugin)
        .add_plugins(DefaultPickingPlugins)
        .add_plugins(crate::cursor::plugin())
        .add_plugins(SpawnerPlugin::<Waymark>::default())
        .add_systems(Startup, add_test_camera)
        .add_systems(Startup, spawn_test_entities)
        .add_systems(Update, draw_test_win::<Waymark>)
        .init_resource::<TestWinPos>();
        // Make sure to finalize and to update once to initialize the UI.
        // Don't use app.run() since it'll loop.
        app.finish();
        app.cleanup();
        app.update();

        let mut win_q = app.world.query_filtered::<Entity, With<PrimaryWindow>>();
        let primary_window = win_q.single(&app.world);
        (app, primary_window)
    }

    #[test]
    //#[ignore = "broken due to Drag imprecision"]
    fn spawner_drag() {
        let (mut app, _) = test_app();

        let drag = Vec2::splat(50.0);
        let start_pos = Vec2::splat(SPAWNER_SIZE / 2.0);
        let end_pos = start_pos + drag;
        app.world.spawn(MockDrag {
            start_pos,
            end_pos,
            button: MouseButton::Left,
            duration: 10.0,
        });
        app.add_systems(Update, MockDrag::update)
            .add_systems(Update, log_debug::<Pointer<DragStart>>)
            .add_systems(Update, log_debug::<Pointer<Drag>>)
            .add_systems(Update, log_debug::<Pointer<DragEnd>>)
            .add_systems(Update, log_debug::<CursorMoved>)
            .add_systems(Update, log_debug::<bevy::input::mouse::MouseButtonInput>)
            .add_systems(First, || {
                debug!("new tick");
            });
        for _ in 0..20 {
            app.update();
        }

        let mut spawner_q = app.world.query_filtered::<(), With<Spawner<Waymark>>>();
        spawner_q.single(&app.world);

        let mut waymark_q = app.world.query_filtered::<&Transform, With<Waymark>>();
        let transform = waymark_q.single(&app.world);
        assert_float_eq!(transform.translation.x, end_pos.x, abs <= 0.0001,);
        assert_float_eq!(transform.translation.y, end_pos.y, abs <= 0.0001,);
    }
}
