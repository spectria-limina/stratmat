//! Waymark tray and associated code.

use bevy::{
    ecs::{component::ComponentId, system::SystemState, world::DeferredWorld},
    prelude::*,
};
use bevy_egui::egui;

use super::{job::Job, Player, PlayerSprite};
use crate::{
    ecs::EntityWorldExts,
    spawner::{self, panel::SpawnerPanel, Spawnable, Spawner},
    widget::egui_context,
};

const SIZE: f32 = 35.0;
const SEP: f32 = 10.0;

impl Spawnable for PlayerSprite {
    const UNIQUE: bool = true;

    fn size() -> Vec2 { Vec2::splat(SIZE) }
    fn sep() -> Vec2 { Vec2::splat(SEP) }

    fn spawner_name(&self) -> std::borrow::Cow<'static, str> {
        format!("{:#?} Spawner", self.job).into()
    }

    fn texture_handle(&self, asset_server: &AssetServer) -> Handle<Image> {
        asset_server.load(self.asset_path())
    }

    fn insert(&self, entity: &mut EntityCommands) { entity.insert((Player {}, *self)); }
}

/// A window with controls to manipulate the waymarks.
#[derive(Debug, Default, Copy, Clone, Component, Reflect)]
pub struct PlayerWindow;

impl PlayerWindow {
    /// [System] that draws the waymark window and handles events.
    ///
    /// Will panic if there is more than one camera.
    pub fn show(world: &mut World) {
        let ctx = egui_context(world);
        let mut state =
            SystemState::<(Query<Entity, With<PlayerWindow>>, Query<&Children>)>::new(world);

        let ewin = egui::Window::new("Players")
            .default_width(4.0 * (PlayerSprite::size() + PlayerSprite::sep()).x);
        ewin.show(&ctx, |ui| {
            let (mut win_q, _parent_q) = state.get_mut(world);
            let win_id = win_q.single_mut();

            let panel = crate::spawner::panel::SpawnerPanel::<PlayerSprite>::new();
            world.entity_mut(win_id).run_instanced_with(
                crate::spawner::panel::SpawnerPanel::<PlayerSprite>::show,
                (ui, panel),
            );

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

        world.commands().queue(move |mut world: &mut World| {
            world.resource_scope(move |world: &mut World, asset_server: Mut<AssetServer>| {
                world.entity_mut(id).with_children(move |window| {
                    window
                        .spawn(SpawnerPanel::<PlayerSprite>::new())
                        .with_children(move |panel| {
                            for job in JOBS {
                                let sprite = PlayerSprite { job: Some(job) };
                                panel.spawn(Spawner::<PlayerSprite>::new(
                                    sprite,
                                    asset_server.load(sprite.asset_path()),
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
                commands.spawn((PlayerWindow, Name::new("Players")))
            });
    }
}

pub fn plugin() -> WaymarkWindowPlugin { WaymarkWindowPlugin }
