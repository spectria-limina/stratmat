//! Waymark tray and associated code.

use bevy::{
    ecs::{component::ComponentId, system::SystemState, world::DeferredWorld},
    prelude::*,
};
use bevy_egui::egui;
use itertools::Itertools;

use super::{job::Job, Player, PlayerSprite, PLAYER_Z};
use crate::{
    ecs::{EntityWorldExts, NestedSystemExts},
    spawner::{self, panel::SpawnerPanel, Spawnable, Spawner},
    widget::{egui_context, Widget, WidgetSystemId},
};

const SIZE: f32 = 35.0;
const SEP: f32 = 10.0;

impl Spawnable for PlayerSprite {
    const UNIQUE: bool = true;
    const Z: f32 = PLAYER_Z;

    fn size() -> Vec2 { Vec2::splat(SIZE) }
    fn sep() -> Vec2 { Vec2::splat(SEP) }

    fn spawner_name(&self) -> std::borrow::Cow<'static, str> { format!("{:#?}", self.job).into() }

    fn insert(&self, entity: &mut EntityCommands) { entity.insert((Player {}, *self)); }
}

/// A window with controls to manipulate the waymarks.
#[derive(Debug, Default, Copy, Clone, Component, Reflect)]
#[component(on_add = Self::on_add)]
pub struct PlayerWindow;

impl PlayerWindow {
    /// [System] that draws the waymark window and handles events.
    ///
    /// Will panic if there is more than one camera.
    pub fn show(world: &mut World) {
        let ctx = egui_context(world);
        let mut state = SystemState::<(
            Query<Entity, With<PlayerWindow>>,
            Query<&Widget, With<SpawnerPanel<PlayerSprite>>>,
            Query<&Children>,
        )>::new(world);

        let ewin = egui::Window::new("Players")
            .default_width(4.0 * (PlayerSprite::size() + PlayerSprite::sep()).x);
        ewin.show(&ctx, |ui| {
            let (mut win_q, panel_q, parent_q) = state.get_mut(world);
            let win_id = win_q.single_mut();

            let panel = panel_q
                .iter_many(parent_q.children(win_id))
                .copied()
                .exactly_one()
                .unwrap();
            panel.show_world(world, ui);

            state.apply(world);
        });
    }

    /// Setup the window.
    pub fn on_add(mut world: DeferredWorld, id: Entity, _: ComponentId) {
        const JOBS: [Job; 8] = [
            Job::Paladin,
            Job::DarkKnight,
            Job::Astrologian,
            Job::Scholar,
            Job::RedMage,
            Job::Bard,
            Job::Pictomancer,
            Job::Dragoon,
        ];

        world.commands().queue(move |world: &mut World| {
            world.resource_scope(move |world: &mut World, asset_server: Mut<AssetServer>| {
                world.entity_mut(id).with_children(move |window| {
                    window
                        .spawn(SpawnerPanel::<PlayerSprite>::new())
                        .with_children(move |panel| {
                            for job in JOBS {
                                let sprite = PlayerSprite { job: Some(job) };
                                panel.spawn(Spawner::<PlayerSprite>::new(
                                    sprite,
                                    sprite.asset_path().into(),
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
        app.add_plugins(spawner::plugin::<PlayerSprite>())
            .add_systems(Update, PlayerWindow::show)
            .add_systems(Startup, |mut commands: Commands| {
                commands.spawn((PlayerWindow, Name::new("Players")));
            });
    }
}

pub fn plugin() -> WaymarkWindowPlugin { WaymarkWindowPlugin }
