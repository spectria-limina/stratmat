use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use super::{Arenas, CommandsDespawnAllArenasExt, CommandsSpawnArenaExt};

#[derive(Component, Debug)]
pub struct ArenaMenu {}

impl ArenaMenu {
    pub fn show(
        mut q: Query<&mut ArenaMenu>,
        arenas: Arenas,
        asset_server: Res<AssetServer>,
        mut contexts: EguiContexts,
        mut commands: Commands,
    ) {
        let ctx = contexts.ctx_mut();
        for mut _menu in &mut q {
            egui::TopBottomPanel::top("top").show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("Arenas", |ui| match arenas.get() {
                        Some(iter) => {
                            for (id, arena) in iter {
                                if ui.button(arena.short_name.clone()).clicked() {
                                    commands.despawn_all_arenas();
                                    commands.spawn_arena(asset_server.get_id_handle(id).unwrap());
                                }
                            }
                        }
                        None => {
                            ui.set_enabled(false);
                        }
                    })
                });
            });
        }
    }
}

#[derive(Default, Copy, Clone, Debug)]
pub struct ArenaMenuPlugin;

impl Plugin for ArenaMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, ArenaMenu::show)
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn(ArenaMenu {});
            });
    }
}
