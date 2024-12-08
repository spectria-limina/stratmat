//! Waymark tray and associated code.

use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use bevy_egui::egui;

use super::job::Job;
use super::PlayerSprite;
use crate::spawner::{self, Spawner, SpawnerWidget};
use crate::widget::{self, egui_context};

/// The size of waymark spawner, in pixels.
const PLAYER_SPAWNER_SIZE: f32 = 35.0;
const PLAYER_SPAWNER_SEP: f32 = 8.0;

/// A window with controls to manipulate the waymarks.
#[derive(Debug, Default, Copy, Clone, Component, Reflect)]
pub struct PlayerWindow;

impl PlayerWindow {
    /// [System] that draws the waymark window and handles events.
    ///
    /// Will panic if there is more than one camera.
    pub fn draw(world: &mut World) {
        let ctx = egui_context(world);
        let mut state = SystemState::<(
            Query<Entity, With<PlayerWindow>>,
            Query<&Children>,
            Query<(Entity, &Spawner<PlayerSprite>)>,
        )>::new(world);

        let ewin = egui::Window::new("Players")
            .default_width(4.0 * (PLAYER_SPAWNER_SIZE + PLAYER_SPAWNER_SEP));
        ewin.show(&ctx, |ui| {
            let (mut win_q, parent_q, spawner_q) = state.get_mut(world);
            let win_id = win_q.single_mut();
            let mut spawners = parent_q
                .children(win_id)
                .iter()
                .filter_map(|&id| spawner_q.get(id).ok())
                .map(|(id, spawner)| (id, spawner.clone()))
                .collect::<Vec<_>>();
            spawners.sort_by_key(|(_, spawner)| spawner.target.job);

            ui.with_layout(
                egui::Layout::left_to_right(egui::Align::Min)
                    .with_main_wrap(true)
                    .with_main_align(egui::Align::Center),
                |ui| {
                    ui.style_mut().spacing.item_spacing = [PLAYER_SPAWNER_SEP; 2].into();
                    for (id, _) in spawners {
                        widget::show::<SpawnerWidget<PlayerSprite>>(world, ui, id);
                    }
                },
            );
            state.apply(world);
        });
    }

    /// Setup the window.
    pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
        let jobs = [
            Job::Paladin,
            Job::DarkKnight,
            Job::Astrologian,
            Job::Scholar,
            Job::RedMage,
            Job::Bard,
            Job::Pictomancer,
            Job::Dragoon,
        ];
        commands
            .spawn((PlayerWindow, Name::new("Player Window")))
            .with_children(|parent| {
                for job in jobs {
                    let sprite = PlayerSprite { job: Some(job) };
                    parent.spawn(Spawner::<PlayerSprite>::new(
                        sprite,
                        asset_server.load(sprite.asset_path()),
                        Vec2::splat(PLAYER_SPAWNER_SIZE),
                    ));
                }
            });
    }
}

/// Plugin for the waymark window.
#[derive(Default, Copy, Clone, Debug)]
pub struct WaymarkWindowPlugin;

impl Plugin for WaymarkWindowPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(spawner::plugin::<PlayerSprite>())
            .add_systems(Update, PlayerWindow::draw)
            .add_systems(Startup, PlayerWindow::setup);
    }
}

pub fn plugin() -> WaymarkWindowPlugin {
    WaymarkWindowPlugin
}
