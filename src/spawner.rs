use std::borrow::Cow;
use std::marker::PhantomData;

use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;
use bevy_egui::egui::Response;
use bevy_egui::{egui, EguiContexts};
use bevy_mod_picking::backend::{HitData, PointerHits};
use bevy_mod_picking::prelude::*;
use itertools::Itertools;
use std::fmt::Debug;

use crate::cursor::EntityCommandsStartDragExt;

/// The alpha (out of 255) of an enabled waymark spawner widget.
const SPAWNER_ALPHA: u8 = 230;
/// The alpha (out of 255) of a disabled waymark spawner widget.
const SPAWNER_DISABLED_ALPHA: u8 = 25;

/// An entity that can be spawned.
pub trait Spawnable: Component + Reflect + TypePath + Clone + PartialEq + Debug {
    fn spawner_name(&self) -> Cow<'static, str>;
    fn texture_handle(&self, asset_server: &AssetServer) -> Handle<Image>;
    fn insert(&self, entity: &mut EntityCommands);
}

/// An entity that can be clicked & dragged to spawn a waymark.
///
/// Rendered using egui, not the normal logic.
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(from_reflect = false)]
pub struct Spawner<E: Spawnable> {
    pub target: E,
    #[reflect(ignore)]
    pub texture_id: egui::TextureId,
}

/// Information required for communication between a [Spawner] and the UI function.
#[derive(Component, Clone, Debug, Default, Reflect)]
pub struct SpawnerUi {
    // Spawner -> UI
    pub enabled: bool,
    // UI -> Spawner
    pub hover_pos: Option<Vec2>,
}

impl<E: Spawnable> Spawner<E> {
    /// System that extracts information from the entity that is needed for updating the UI.
    pub fn extract_ui(mut q: Query<(&Spawner<E>, &mut SpawnerUi)>, waymark_q: Query<&E>) {
        for (spawner, mut ui) in &mut q {
            ui.enabled = !waymark_q.iter().contains(&spawner.target);
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
        spawner_q: Query<(&Spawner<E>, &SpawnerUi)>,
        camera_q: Query<(&Camera, &GlobalTransform)>,
        mut commands: Commands,
    ) {
        let id = ev.listener();
        debug!("starting drag on spawner {id:?}");
        let Ok((spawner, ui)) = spawner_q.get(id) else {
            debug!("but it doesn't exist");
            return;
        };
        if !ui.enabled {
            debug!("but it's disabled");
            return;
        }
        commands.spawn(SpawnerBundle {
            name: Name::new(format!("Spawner for {}", spawner.target.spawner_name())),
            spawner: spawner.clone(),
            ui: ui.clone(),
            pickable: default(),
            drag_start: On::<Pointer<DragStart>>::run(Self::drag_start),
        });

        let mut entity = commands.entity(id);
        entity.remove::<SpawnerBundle<E>>();

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

    /// System that takes hover data from the UI and uses it to generate pointer events.
    pub fn generate_hits(
        q: Query<(Entity, &SpawnerUi), With<Spawner<E>>>,
        mut pointer_ev: EventWriter<PointerHits>,
    ) {
        for (id, ui) in &q {
            if let Some(pos) = ui.hover_pos {
                pointer_ev.send(PointerHits::new(
                    PointerId::Mouse,
                    vec![(id, HitData::new(id, 0.0, Some(pos.extend(0.0)), None))],
                    // egui is at depth 1_000_000, we need to be in front of that.
                    1_000_001.0,
                ));
            }
        }
    }
}

impl SpawnerUi {
    /// Render this entity on the [Ui], updating the [SpawnerUi] component based on egui state.
    pub fn show<E: Spawnable>(
        &mut self,
        ui: &mut egui::Ui,
        spawner: &Spawner<E>,
        size: Vec2,
    ) -> Response {
        let resp = ui.add(
            egui::Image::new((spawner.texture_id, egui::Vec2::new(size.x, size.y)))
                .tint(egui::Color32::from_white_alpha(if self.enabled {
                    SPAWNER_ALPHA
                } else {
                    SPAWNER_DISABLED_ALPHA
                }))
                .sense(egui::Sense::drag()),
        );

        self.hover_pos = if resp.hovered() {
            let egui::Pos2 { x, y } = resp.hover_pos().unwrap();
            Some(Vec2::new(x, y))
        } else {
            None
        };
        resp
    }
}

/// Bundle of components for a [Spawner].
#[derive(Bundle)]
pub struct SpawnerBundle<E: Spawnable> {
    pub name: Name,
    pub spawner: Spawner<E>,
    pub ui: SpawnerUi,
    pub pickable: PickableBundle,
    pub drag_start: On<Pointer<DragStart>>,
}

impl<E: Spawnable> SpawnerBundle<E> {
    pub fn new(entity: E, asset_server: &AssetServer, contexts: &mut EguiContexts) -> Self {
        let texture = entity.texture_handle(asset_server);
        Self {
            name: Name::new(format!("Spawner for {}", entity.spawner_name())),
            spawner: Spawner {
                target: entity,
                texture_id: contexts.add_image(texture),
            },
            ui: default(),
            pickable: default(),
            drag_start: On::<Pointer<DragStart>>::run(Spawner::<E>::drag_start),
        }
    }
}

/// Plugin for spawner support
#[derive(Copy, Clone, Debug)]
pub struct SpawnerPlugin<E> {
    _phantom: PhantomData<E>,
}

impl<E> Default for SpawnerPlugin<E> {
    fn default() -> Self {
        Self {
            _phantom: default(),
        }
    }
}

impl<E: Spawnable> Plugin for SpawnerPlugin<E> {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, Spawner::<E>::extract_ui)
            .add_systems(PostUpdate, Spawner::<E>::generate_hits)
            .register_type::<Spawner<E>>()
            .register_type::<SpawnerUi>();
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
    use crate::testing::*;
    use crate::waymark::Waymark;

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

    fn draw_test_win<E: Spawnable>(
        mut contexts: EguiContexts,
        mut spawner_q: Query<(&Spawner<E>, &mut SpawnerUi)>,
        pos: Res<TestWinPos>,
    ) {
        let (spawner, ref mut spawner_ui) = spawner_q.iter_mut().next().unwrap();
        egui::Area::new("test")
            .fixed_pos(pos.0)
            .show(contexts.ctx_mut(), |ui| {
                spawner_ui.show(ui, spawner, Vec2::splat(SPAWNER_SIZE));
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
    pub fn spawner_hover_pos() {
        let (mut app, primary_window) = test_app();

        app.world.send_event(CursorMoved {
            window: primary_window,
            position: Vec2::new(-100.0, -100.0),
        });
        app.update();

        let desc = "cursor at (-100, -100), spawner with top left at (0, 0)";

        let mut ui_q = app.world.query::<&SpawnerUi>();
        let ui = ui_q.single(&app.world);
        assert_eq!(
            ui.hover_pos, None,
            "with {desc}: ui.hover_pos = {:?}; want None",
            ui.hover_pos,
        );

        let target = Vec2::new(20.0, 20.0);
        app.world.send_event(CursorMoved {
            window: primary_window,
            position: target,
        });
        app.update();

        let ui = ui_q.single(&app.world);
        assert!(
            ui.hover_pos.is_some(),
            "with {desc}: ui.hover_pos = None; want {:?}",
            Some(target)
        );

        let Vec2 {
            x: hover_x,
            y: hover_y,
        } = ui.hover_pos.unwrap();
        assert_float_eq!(
            hover_x,
            target.x,
            abs <= 0.0001,
            "with {desc}: check ui.hover_pos.x",
        );
        assert_float_eq!(
            hover_y,
            target.y,
            abs <= 0.0001,
            "with {desc}: check ui.hover_pos.y",
        );
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
