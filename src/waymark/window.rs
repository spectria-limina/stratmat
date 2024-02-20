//! Waymark tray and associated code.

use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_egui::egui::{TextEdit, Ui};
use bevy_egui::{egui, EguiClipboard, EguiContexts};

use super::{CommandsDespawnAllWaymarksExt, CommandsSpawnWaymarksFromPresetExt, Preset, Waymark};
use crate::arena::Arena;
use crate::ecs::RegistryExt;
use crate::spawner::{Spawner, SpawnerBundle, SpawnerPlugin, SpawnerUi};

/// The size of waymark spawner, in pixels.
const WAYMARK_SPAWNER_SIZE: f32 = 40.0;

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
        mut spawner_q: Query<(&mut SpawnerUi, &Spawner<Waymark>)>,
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
                    commands.run(Self::export_to_clipboard)
                }
                if ui.button("Clear").clicked() {
                    commands.despawn_all_waymarks()
                }
            });

            let mut spawners: HashMap<_, _> = spawner_q
                .iter_mut()
                .map(|(ui, spawner)| (spawner.target, (ui, spawner)))
                .collect();
            let show = |&mut (ref mut spawner_ui, spawner): &mut (
                Mut<'_, SpawnerUi>,
                &Spawner<Waymark>,
            ),
                        ui: &mut Ui| {
                spawner_ui.show(ui, spawner, Vec2::splat(WAYMARK_SPAWNER_SIZE))
            };

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
        arena_q: Query<&Handle<Arena>>,
        arenas: Res<Assets<Arena>>,
        mut clipboard: ResMut<EguiClipboard>,
    ) {
        let arena = arenas.get(arena_q.single()).unwrap();
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
        commands.spawn(WaymarkWindow::default());
        // TODO: Make spawners children of the window?
        for waymark in enum_iterator::all::<Waymark>() {
            commands.spawn(SpawnerBundle::<Waymark>::new(
                waymark,
                &asset_server,
                &mut contexts,
            ));
        }
    }
}

/// Plugin for the waymark window.
#[derive(Default, Copy, Clone, Debug)]
pub struct WaymarkWindowPlugin {}

impl Plugin for WaymarkWindowPlugin {
    fn build(&self, app: &mut App) {
        app.register(WaymarkWindow::export_to_clipboard)
            .add_plugins(SpawnerPlugin::<Waymark>::default())
            .add_systems(Update, WaymarkWindow::draw)
            .add_systems(Startup, WaymarkWindow::setup);
    }
}
