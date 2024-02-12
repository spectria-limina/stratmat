//! Waymark tray and associated code.

use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_egui::egui::{Response, TextEdit, Ui};
use bevy_egui::{egui, EguiClipboard, EguiContexts};
use bevy_mod_picking::backend::{HitData, PointerHits};
use bevy_mod_picking::prelude::*;
use itertools::Itertools;

use super::{CommandsDespawnAllWaymarksExt, CommandsSpawnWaymarksFromPresetExt, Preset, Waymark};
use crate::arena::ArenaData;
use crate::systems::RegistryExt;
use crate::waymark::EntityCommandsInsertWaymarkExt;

/// The size of waymark spawner, in pixels.
const WAYMARK_SPAWNER_SIZE: f32 = 40.0;
/// The alpha (out of 255) of an enabled waymark spawner widget.
const WAYMARK_SPAWNER_ALPHA: u8 = 230;
/// The alpha (out of 255) of a disabled waymark spawner widget.
const WAYMARK_SPAWNER_DISABLED_ALPHA: u8 = 25;

/// An entity that can be clicked & dragged to spawn a waymark.
///
/// Rendered using egui, not the normal logic.
#[derive(Debug, Clone, Component, Reflect)]
#[reflect(from_reflect = false)]
pub struct Spawner {
    waymark: Waymark,
    #[reflect(ignore)]
    texture_id: egui::TextureId,
}

/// Information required for communication between a [Spawner] and the UI function.
#[derive(Component, Clone, Debug, Default, Reflect)]
pub struct SpawnerUi {
    // Spawner -> UI
    pub enabled: bool,
    // UI -> Spawner
    pub center: Vec2,
    pub hover_pos: Option<Vec2>,
}

impl Spawner {
    /// System that extracts information from the entity that is needed for updating the UI.
    pub fn extract_ui(mut q: Query<(&Spawner, &mut SpawnerUi)>, waymark_q: Query<&Waymark>) {
        for (spawner, mut ui) in &mut q {
            ui.enabled = !waymark_q.iter().contains(&spawner.waymark);
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
        spawner_q: Query<(&Spawner, &SpawnerUi)>,
        camera_q: Query<(&Camera, &GlobalTransform)>,
        mut commands: Commands,
    ) {
        log::trace!("{}::Spawner::drag_start", module_path!());
        let id = ev.listener();
        let Ok((spawner, ui)) = spawner_q.get(id) else {
            log::debug!("skipping spawner drag because entity no longer exists");
            return;
        };
        if !ui.enabled {
            log::debug!(
                "skipping spawner drag for waymark {:?} because the spawner is disabled",
                spawner.waymark
            );
            return;
        }
        commands.spawn(SpawnerBundle {
            name: Name::new(format!("Spawner for {}", spawner.waymark.name())),
            spawner: spawner.clone(),
            ui: ui.clone(),
            pickable: default(),
            drag_start: On::<Pointer<DragStart>>::run(Self::drag_start),
        });

        let mut entity = commands.entity(id);
        entity.remove::<SpawnerBundle>();

        let (camera, camera_transform) = camera_q.single();
        let hit_position = ev.hit.position.unwrap().truncate();
        let translation = camera
            .viewport_to_world_2d(camera_transform, hit_position)
            .unwrap()
            .extend(0.0);
        log::debug!(
            "spawner spawning waymark {:?} at {translation} (from hit position: {hit_position})",
            spawner.waymark,
        );

        entity.insert_waymark(spawner.waymark, None);
        entity.insert(Transform::from_translation(translation));
    }

    /// System that takes hover data from the UI and uses it to generate pointer events.
    pub fn generate_hits(
        q: Query<(Entity, &SpawnerUi), With<Spawner>>,
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

    pub fn setup(
        mut commands: Commands,
        asset_server: Res<AssetServer>,
        mut contexts: EguiContexts,
    ) {
        for waymark in enum_iterator::all::<Waymark>() {
            commands.spawn(SpawnerBundle::new(waymark, &asset_server, &mut contexts));
        }
    }
}

impl SpawnerUi {
    /// Render this entity on the [Ui], updating the [SpawnerUi] component based on egui state.
    pub fn show(&mut self, ui: &mut egui::Ui, spawner: &Spawner) -> Response {
        let resp = ui.add(
            egui::Image::new((
                spawner.texture_id,
                egui::Vec2::new(WAYMARK_SPAWNER_SIZE, WAYMARK_SPAWNER_SIZE),
            ))
            .tint(egui::Color32::from_white_alpha(if self.enabled {
                WAYMARK_SPAWNER_ALPHA
            } else {
                WAYMARK_SPAWNER_DISABLED_ALPHA
            }))
            .sense(egui::Sense::drag()),
        );

        let egui::Pos2 { x, y } = resp.rect.center();
        self.center = Vec2::new(x, y);

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
pub struct SpawnerBundle {
    pub name: Name,
    pub spawner: Spawner,
    pub ui: SpawnerUi,
    pub pickable: PickableBundle,
    pub drag_start: On<Pointer<DragStart>>,
}

impl SpawnerBundle {
    pub fn new(waymark: Waymark, asset_server: &AssetServer, contexts: &mut EguiContexts) -> Self {
        Self {
            name: Name::new(format!("Spawner for {}", waymark.name())),
            spawner: Spawner {
                waymark,
                texture_id: contexts.add_image(waymark.asset_handle(asset_server)),
            },
            ui: default(),
            pickable: default(),
            drag_start: On::<Pointer<DragStart>>::run(Spawner::drag_start),
        }
    }
}

/// A window with controls to manipulate the waymarks.
#[derive(Debug, Default, Component)]
pub struct WaymarkWindow {
    preset_name: String,
}

impl WaymarkWindow {
    /// [System] that draws the waymark window and handles events.
    ///
    /// Will panic if there is more than one camera.
    pub fn draw(
        mut win_q: Query<&mut WaymarkWindow>,
        mut spawner_q: Query<(&mut SpawnerUi, &Spawner)>,
        mut commands: Commands,
        mut contexts: EguiContexts,
        clipboard: Res<EguiClipboard>,
    ) {
        let mut win = win_q.single_mut();

        let ewin = egui::Window::new("Waymarks").default_width(4.0 * WAYMARK_SPAWNER_SIZE);
        ewin.show(contexts.ctx_mut(), |ui| {
            ui.horizontal(|ui| {
                ui.label("Preset: ");
                ui.add(TextEdit::singleline(&mut win.preset_name).desired_width(100.0));
            });
            ui.horizontal(|ui| {
                if ui.button("Import").clicked() {
                    if let Some(contents) = clipboard.get_contents() {
                        match serde_json::from_str::<Preset>(&contents) {
                            Ok(preset) => {
                                win.preset_name = preset.name.clone();
                                commands.despawn_all_waymarks();
                                commands.spawn_waymarks_from_preset(preset);
                                log::info!(
                                    "Imported waymark preset '{}' from the clipboard",
                                    win.preset_name
                                );
                            }
                            Err(e) => {
                                log::info!("Unable to import waymark preset: {}", e);
                            }
                        }
                    } else {
                        log::info!("Unable to import waymark preset: clipboard is empty")
                    }
                }
                if ui.button("Export").clicked() {
                    commands.run(Self::export_to_clipboard)
                }
                if ui.button("Clear").clicked() {
                    commands.despawn_all_waymarks()
                }
            });

            let mut spawners: HashMap<_, _> = spawner_q
                .iter_mut()
                .map(|(ui, spawner)| (spawner.waymark, (ui, spawner)))
                .collect();
            let show = |&mut (ref mut spawner_ui, spawner): &mut (Mut<'_, SpawnerUi>, &Spawner),
                        ui: &mut Ui| { spawner_ui.show(ui, spawner) };

            ui.separator();
            ui.horizontal(|ui| {
                show(spawners.get_mut(&Waymark::One).unwrap(), ui);
                show(spawners.get_mut(&Waymark::Two).unwrap(), ui);
                show(spawners.get_mut(&Waymark::Three).unwrap(), ui);
                show(spawners.get_mut(&Waymark::Four).unwrap(), ui);
            });
            ui.horizontal(|ui| {
                show(spawners.get_mut(&Waymark::A).unwrap(), ui);
                show(spawners.get_mut(&Waymark::B).unwrap(), ui);
                show(spawners.get_mut(&Waymark::C).unwrap(), ui);
                show(spawners.get_mut(&Waymark::D).unwrap(), ui);
            });
        });
    }

    /// [System] that exports the currently-spawned waymarks to the clipboard.
    pub fn export_to_clipboard(
        win_q: Query<&WaymarkWindow>,
        waymarks_q: Query<(&Waymark, &Transform)>,
        arena_q: Query<&ArenaData>,
        mut clipboard: ResMut<EguiClipboard>,
    ) {
        let arena = arena_q.single();
        let preset = Preset {
            name: win_q.single().preset_name.clone(),
            map_id: arena.map_id,
            waymarks: waymarks_q
                .iter()
                .map(|(&waymark, transform)| (waymark, waymark.to_entry(transform, arena.offset)))
                .collect(),
        };
        match serde_json::to_string(&preset) {
            Ok(json) => {
                clipboard.set_contents(&json);
                log::info!("Exported waymark preset '{}' to the clipboard", preset.name)
            }
            Err(e) => log::error!("Unable to serialize waymark preset for export: {e}"),
        }
    }

    /// Setup the window.
    pub fn setup(mut commands: Commands) {
        commands.spawn(WaymarkWindow::default());
    }
}

/// Plugin for the waymark window.
#[derive(Debug, Default, Copy, Clone)]
pub struct WaymarkUiPlugin {
    #[cfg(test)]
    for_test: bool,
}

impl WaymarkUiPlugin {
    #[cfg(test)]
    fn new_for_test() -> Self {
        Self { for_test: true }
    }
}

impl Plugin for WaymarkUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, Spawner::extract_ui)
            .add_systems(PostUpdate, Spawner::generate_hits)
            .register(WaymarkWindow::export_to_clipboard)
            .register_type::<Spawner>()
            .register_type::<SpawnerUi>()
            // Ensure that deferred commands are run after [DragStart] events but before [Drag] events.
            // This is required to allow entities to transform themselves in response to [DragStart] and still pick up [Drag] the same frame.
            .add_systems(
                PreUpdate,
                apply_deferred
                    .after(bevy_eventlistener_core::event_dispatcher::EventDispatcher::<Pointer<DragStart>>::cleanup)
                    .before(bevy_eventlistener_core::event_dispatcher::EventDispatcher::<Pointer<Drag>>::build)
            );

        #[allow(unused_mut, unused_assignments)]
        let mut for_test = false;

        #[cfg(test)]
        {
            for_test = self.for_test;
        }

        if !for_test {
            app.add_systems(Update, WaymarkWindow::draw)
                .add_systems(Startup, Spawner::setup)
                .add_systems(Startup, WaymarkWindow::setup);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::testing::*;

    use bevy::app::ScheduleRunnerPlugin;
    use bevy::render::settings::{RenderCreation, WgpuSettings};
    use bevy::render::RenderPlugin;
    use bevy::window::PrimaryWindow;
    use bevy::winit::WinitPlugin;
    use bevy_egui::egui::Pos2;
    use bevy_egui::EguiPlugin;
    use bevy_egui::{egui, EguiContexts};

    use bevy_mod_picking::DefaultPickingPlugins;
    use float_eq::assert_float_eq;

    #[derive(Default, Resource)]
    struct TestWinPos(egui::Pos2);

    fn draw_test_win(
        mut contexts: EguiContexts,
        mut spawner_q: Query<(&Spawner, &mut SpawnerUi)>,
        pos: Res<TestWinPos>,
    ) {
        let (spawner, ref mut spawner_ui) = spawner_q.iter_mut().next().unwrap();
        egui::Area::new("test")
            .fixed_pos(pos.0)
            .show(contexts.ctx_mut(), |ui| {
                spawner_ui.show(ui, spawner);
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
        .add_plugins(WaymarkUiPlugin::new_for_test())
        .add_systems(Startup, add_test_camera)
        .add_systems(Startup, spawn_test_entities)
        .add_systems(Update, draw_test_win)
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
    pub fn spawner_center() {
        let (mut app, _) = test_app();

        let mut ui_q = app.world.query::<&SpawnerUi>();
        let ui = ui_q.single(&app.world);
        assert_float_eq!(ui.center.x, WAYMARK_SPAWNER_SIZE / 2.0, abs <= 0.0001,);
        assert_float_eq!(ui.center.y, WAYMARK_SPAWNER_SIZE / 2.0, abs <= 0.0001,);

        let pos = Pos2::new(250.0, -500.0);
        app.world.resource_mut::<TestWinPos>().0 = pos;
        app.update();

        let ui = ui_q.single(&app.world);
        assert_float_eq!(
            ui.center.x,
            pos.x + WAYMARK_SPAWNER_SIZE / 2.0,
            abs <= 0.0001,
        );
        assert_float_eq!(
            ui.center.y,
            pos.y + WAYMARK_SPAWNER_SIZE / 2.0,
            abs <= 0.0001,
        );
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
        let start_pos = Vec2::splat(WAYMARK_SPAWNER_SIZE / 2.0);
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
                log::debug!("new tick");
            });
        for _ in 0..20 {
            app.update();
        }

        let mut spawner_q = app.world.query_filtered::<(), With<Spawner>>();
        spawner_q.single(&app.world);

        let mut waymark_q = app.world.query_filtered::<&Transform, With<Waymark>>();
        let transform = waymark_q.single(&app.world);
        assert_float_eq!(transform.translation.x, end_pos.x, abs <= 0.0001,);
        assert_float_eq!(transform.translation.y, end_pos.y, abs <= 0.0001,);
    }
}

// TODO: Put this somewhere better lol.
fn log_debug<E: std::fmt::Debug + Event>(mut events: EventReader<E>) {
    for ev in events.read() {
        debug!("{ev:?}");
    }
}
