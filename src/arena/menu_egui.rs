use bevy::prelude::*;
use bevy_egui::{
    egui::{self, RichText},
    EguiContexts,
};

use super::{despawn_all_arenas, spawn_arena, ArenaListing, ArenaMeta};
use crate::{
    asset::OptionalGlobalAsset,
    egui::{
        menu::TopMenu,
        widget::{widget, InitWidget, WidgetCtx},
    },
};

#[derive(Component, Debug)]
#[require(InitWidget(|| widget!()))]
pub struct ArenaMenu {}

impl ArenaMenu {
    pub fn show(
        WidgetCtx {
            ns: _ns,
            id: _id,
            ui,
        }: WidgetCtx,
        arenas: OptionalGlobalAsset<ArenaListing>,
        assets: Res<Assets<ArenaMeta>>,
        mut commands: Commands,
    ) {
        if let Some(ref listing) = arenas.option() {
            Self::submenu(ui, listing, &assets, &mut commands);
        } else {
            ui.menu_button("Arenas", |ui| {
                ui.label(RichText::new("Loading...").italics())
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
        app.add_systems(
            Startup,
            |top: Single<Entity, With<TopMenu>>, mut commands: Commands| {
                commands.entity(*top).with_child(ArenaMenu {});
            },
        );
    }
}

pub fn plugin() -> ArenaMenuPlugin { ArenaMenuPlugin }
