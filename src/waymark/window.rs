//! Waymark tray and associated code.

use bevy::{
    ecs::{component::ComponentId, system::SystemState, world::DeferredWorld},
    prelude::*,
};
use bevy_egui::{egui, egui::TextEdit, EguiClipboard};

use super::{insert_waymark, Preset, Waymark};
use crate::{
    arena::Arena,
    ecs::EntityWorldExts,
    spawner::{self, panel::SpawnerPanel, Spawnable, Spawner},
    widget::egui_context,
};

const SPAWNER_SIZE: f32 = 40.0;
const SPAWNER_SEP: f32 = 5.0;

impl Spawnable for Waymark {
    const UNIQUE: bool = true;

    fn size() -> Vec2 { Vec2::splat(SPAWNER_SIZE) }
    fn sep() -> Vec2 { Vec2::splat(SPAWNER_SEP) }

    fn spawner_name(&self) -> std::borrow::Cow<'static, str> { self.name().into() }

    fn texture_handle(&self, asset_server: &AssetServer) -> Handle<Image> {
        self.asset_handle(asset_server)
    }

    fn insert(&self, entity: &mut bevy::ecs::system::EntityCommands) {
        entity.queue(insert_waymark(*self, None));
    }
}

/// A window with controls to manipulate the waymarks.
#[derive(Debug, Default, Clone, Component, Reflect)]
pub struct WaymarkWindow {
    preset_name: String,
}

impl WaymarkWindow {
    /// [System] that draws the waymark window and handles events.
    pub fn show(world: &mut World) {
        let ctx = egui_context(world);
        let mut state = SystemState::<(
            Query<(Entity, &mut WaymarkWindow)>,
            Query<&Children>,
            Commands,
            ResMut<EguiClipboard>,
        )>::new(world);

        let ewin =
            egui::Window::new("Waymarks").default_width(4.0 * (Waymark::size() + Waymark::sep()).x);
        ewin.show(&ctx, |ui| {
            let (mut win_q, _parent_q, mut commands, mut clipboard) = state.get_mut(world);
            let (win_id, mut win) = win_q.single_mut();

            ui.horizontal(|ui| {
                ui.label("Preset Name: ");
                ui.add(TextEdit::singleline(&mut win.preset_name).desired_width(80.0));
            });
            ui.horizontal(|ui| {
                if ui.button("Import").clicked() {
                    Self::import_from_clipboard(
                        &mut win.preset_name,
                        &mut clipboard,
                        &mut commands,
                    );
                }
                if ui.button("Export").clicked() {
                    commands.run_system_cached(Self::export_to_clipboard);
                }
                if ui.button("Clear").clicked() {
                    commands.run_system_cached(Waymark::despawn_all);
                }
            });
            #[cfg(target_arch = "wasm32")]
            ui.label(
                bevy_egui::egui::RichText::new("To paste, press Ctrl-C then click Import.")
                    .italics(),
            );
            ui.separator();

            let panel = SpawnerPanel::<Waymark>::new();
            world
                .entity_mut(win_id)
                .run_instanced_with(SpawnerPanel::<Waymark>::show, (ui, panel));

            state.apply(world);
        });
    }

    fn import_from_clipboard(
        preset_name: &mut String,
        clipboard: &mut EguiClipboard,
        commands: &mut Commands,
    ) {
        let Some(contents) = clipboard.get_contents() else {
            info!("Unable to import waymarks: clipboard unavailable");
            return;
        };

        if contents.is_empty() {
            info!("Unable to import waymarks: clipboard is empty (or unavailable)");
            return;
        }

        match serde_json::from_str::<Preset>(&contents) {
            Ok(preset) => {
                *preset_name = preset.name.clone();
                commands.run_system_cached(Waymark::despawn_all);
                Waymark::spawn_from_preset(commands, preset);
                info!(
                    "Imported waymark preset '{}' from the clipboard",
                    preset_name
                );
            }
            Err(e) => {
                info!("Unable to import waymarks: invalid preset: {}", e);
            }
        }
    }

    /// [System] that exports the currently-spawned waymarks to the clipboard.
    pub fn export_to_clipboard(
        win_q: Query<&WaymarkWindow>,
        waymarks_q: Query<(&Waymark, &Transform)>,
        arena: Single<&Arena>,
        mut clipboard: ResMut<EguiClipboard>,
    ) {
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
    pub fn on_add(mut world: DeferredWorld, id: Entity, _: ComponentId) {
        world.commands().queue(move |world: &mut World| {
            world.resource_scope(move |world: &mut World, asset_server: Mut<AssetServer>| {
                world.entity_mut(id).with_children(move |window| {
                    window
                        .spawn(SpawnerPanel::<Waymark>::new())
                        .with_children(move |panel| {
                            for waymark in enum_iterator::all::<Waymark>() {
                                panel.spawn(Spawner::<Waymark>::new(
                                    waymark,
                                    asset_server.load(waymark.asset_path()),
                                ));
                            }
                        });
                });
            });
        });
    }
}

/// Plugin for the waymark window.
#[derive(Default, Copy, Clone, Debug)]
pub struct WaymarkWindowPlugin;

impl Plugin for WaymarkWindowPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(spawner::plugin::<Waymark>())
            .add_systems(Update, WaymarkWindow::show)
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn((WaymarkWindow::default(), Name::new("Waymarks")))
            });
    }
}

pub fn plugin() -> WaymarkWindowPlugin { WaymarkWindowPlugin }
