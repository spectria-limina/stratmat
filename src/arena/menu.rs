use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::asset::lifecycle::GlobalAsset;

use super::{despawn_all_arenas, spawn_arena, ArenaListing, ArenaMeta};

#[derive(Component, Debug)]
pub struct ArenaMenu {}

impl ArenaMenu {
    pub fn show(
        mut q: Query<&mut ArenaMenu>,
        arenas: Option<Single<&GlobalAsset<ArenaListing>>>,
        assets: Res<Assets<ArenaMeta>>,
        mut contexts: EguiContexts,
        mut commands: Commands,
    ) {
        let ctx = contexts.ctx_mut();
        for mut _menu in &mut q {
            egui::TopBottomPanel::top("top").show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    if let Some(ref listing) = arenas {
                        Self::submenu(ui, listing, &assets, &mut commands);
                    }
                });
            });
        }
    }

    fn submenu(
        ui: &mut egui::Ui,
        listing: &ArenaListing,
        assets: &Assets<ArenaMeta>,
        commands: &mut Commands,
    ) {
        ui.menu_button(listing.name.clone(), |ui| {
            for subdir in &listing.subdirs {
                Self::submenu(ui, subdir, assets, commands);
            }
            if !listing.subdirs.is_empty() && !listing.contents.is_empty() {
                ui.separator();
            }
            for handle in &listing.contents {
                let Some(arena) = assets.get(handle) else {
                    error!("arena listing's contents not fully loaded");
                    continue;
                };
                if ui.button(arena.short_name.clone()).clicked() {
                    commands.run_system_cached(despawn_all_arenas);
                    commands.run_system_cached_with(spawn_arena, arena.clone());
                }
            }
        });
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

pub fn plugin() -> ArenaMenuPlugin {
    ArenaMenuPlugin
}
