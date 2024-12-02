//! Waymark tray and associated code.

use bevy::ecs::system::SystemState;
use bevy::prelude::*;

use bevy::utils::hashbrown::HashMap;
use bevy_egui::egui::TextEdit;
use bevy_egui::{egui, EguiClipboard, EguiContexts};

use super::{Preset, Waymark};
use crate::arena::{Arena, ArenaBackground};
use crate::spawner::{Spawner, SpawnerBundle, SpawnerPlugin, SpawnerWidget};
use crate::widget::{self, egui_context};

/// The size of waymark spawner, in pixels.
const WAYMARK_SPAWNER_SIZE: f32 = 40.0;

/// A window with controls to manipulate the waymarks.
#[derive(Debug, Default, Clone, Component)]
pub struct WaymarkWindow {
    preset_name: String,
}

impl WaymarkWindow {
    /// [System] that draws the waymark window and handles events.
    ///
    /// Will panic if there is more than one camera.
    pub fn draw(world: &mut World) {
        let ctx = egui_context(world);
        let mut state = SystemState::<(
            Query<(Entity, &mut WaymarkWindow)>,
            Query<(Entity, &Parent, &Spawner<Waymark>)>,
            Commands,
            ResMut<EguiClipboard>,
        )>::new(world);

        let ewin = egui::Window::new("Waymarks").default_width(4.0 * WAYMARK_SPAWNER_SIZE);
        ewin.show(&ctx, |ui| {
            let (mut win_q, mut spawner_q, mut commands, mut clipboard) = state.get_mut(world);
            let (id, mut win) = win_q.single_mut();

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
                                commands.run_system_cached(Waymark::despawn_all);
                                Waymark::spawn_from_preset(&mut commands, preset);
                                info!(
                                    "Imported waymark preset '{}' from the clipboard",
                                    win.preset_name
                                );
                            }
                            Err(e) => {
                                info!("Unable to import waymark preset: {}", e);
                            }
                        }
                    } else {
                        info!("Unable to import waymark preset: clipboard is empty")
                    }
                }
                if ui.button("Export").clicked() {
                    commands.run_system_cached(Self::export_to_clipboard);
                }
                if ui.button("Clear").clicked() {
                    commands.run_system_cached(Waymark::despawn_all);
                }
            });

            let spawners: HashMap<_, _> = spawner_q
                .iter_mut()
                .filter_map(|(spawner_id, parent, spawner)| {
                    (parent.get() == id).then_some((spawner.target, spawner_id))
                })
                .collect();

            ui.separator();
            ui.horizontal(|ui| {
                for waymark in [Waymark::One, Waymark::Two, Waymark::Three, Waymark::Four] {
                    widget::show::<SpawnerWidget<Waymark>>(world, ui, spawners[&waymark]);
                }
            });
            ui.horizontal(|ui| {
                for waymark in [Waymark::A, Waymark::B, Waymark::C, Waymark::D] {
                    let _spawner = spawners[&waymark];
                    widget::show::<SpawnerWidget<Waymark>>(world, ui, spawners[&waymark]);
                }
            });
            state.apply(world);
        });
    }

    /// [System] that exports the currently-spawned waymarks to the clipboard.
    pub fn export_to_clipboard(
        win_q: Query<&WaymarkWindow>,
        waymarks_q: Query<(&Waymark, &Transform)>,
        arena_q: Query<&ArenaBackground>,
        arenas: Res<Assets<Arena>>,
        mut clipboard: ResMut<EguiClipboard>,
    ) {
        let arena = arenas.get(&arena_q.single().handle).unwrap();
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
                info!("Exported waymark preset '{}' to the clipboard", preset.name)
            }
            Err(e) => error!("Unable to serialize waymark preset for export: {e}"),
        }
    }

    /// Setup the window.
    pub fn setup(
        mut commands: Commands,
        asset_server: Res<AssetServer>,
        mut contexts: EguiContexts,
    ) {
        commands
            .spawn(WaymarkWindow::default())
            .with_children(|parent| {
                for waymark in enum_iterator::all::<Waymark>() {
                    parent
                        .spawn(SpawnerBundle::<Waymark>::new(
                            waymark,
                            asset_server.load(waymark.asset_path()),
                            Vec2::splat(WAYMARK_SPAWNER_SIZE),
                            &mut contexts,
                        ))
                        .observe(Spawner::<Waymark>::start_drag);
                }
            });
    }
}

/// Plugin for the waymark window.
#[derive(Default, Copy, Clone, Debug)]
pub struct WaymarkWindowPlugin {}

impl Plugin for WaymarkWindowPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SpawnerPlugin::<Waymark>::default())
            .add_systems(Update, WaymarkWindow::draw)
            .add_systems(Startup, WaymarkWindow::setup);
    }
}
