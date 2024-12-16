use avian2d::prelude::Collider;
use bevy::{
    ecs::{component::ComponentId, world::DeferredWorld},
    prelude::*,
};
use job::Job;

use crate::{
    drag::Draggable,
    image::{DrawImage, DrawImageKind},
    spawner::Spawnable,
};

pub mod job;

#[cfg(feature = "egui")]
mod window_egui;
pub mod window {
    #[cfg(feature = "egui")]
    pub use super::window_egui::*;
}

/// The size of a player icon.
const PLAYER_SPRITE_SIZE: f32 = 2.0;
const PLAYER_COLLIDER_SIZE: f32 = 0.001;
const PLAYER_Z: f32 = 500.0;

#[derive(Copy, Clone, Hash, PartialEq, Eq, Ord, PartialOrd, Component, Reflect)]
#[require(Transform(|| Transform::from_xyz(0.0, 0.0, PLAYER_Z)))]
#[require(Collider(|| Collider::circle(PLAYER_COLLIDER_SIZE)))]
#[require(Draggable, PlayerSprite)]
#[component(on_add = Self::on_add)]
pub struct Player {}

impl Player {
    pub fn on_add(mut world: DeferredWorld, id: Entity, _: ComponentId) {
        let sprite = *world.get::<PlayerSprite>(id).unwrap();
        if let Some(job) = sprite.job {
            let name = job.to_string();
            world.commands().entity(id).insert_if_new(Name::new(name));
        }
        world.commands().entity(id).insert(DrawImage::new(
            sprite.asset_path().into(),
            Vec2::splat(PLAYER_SPRITE_SIZE),
            DrawImageKind::Sprite,
        ));
    }
}

#[derive(Copy, Default, Clone, Hash, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(Component, Reflect)]
pub struct PlayerSprite {
    pub job: Option<Job>,
}

impl PlayerSprite {
    pub fn update_sprites(
        q: Query<(Entity, &PlayerSprite, &DrawImage), (Changed<PlayerSprite>, With<Player>)>,
        mut commands: Commands,
    ) {
        for (id, sprite, draw) in &q {
            commands.entity(id).insert((DrawImage::new(
                sprite.asset_path().into(),
                Vec2::splat(PLAYER_SPRITE_SIZE),
                draw.kind,
            ),));
        }
    }

    pub fn asset_path(self) -> &'static str {
        self.job
            .map_or(Job::none_asset_path(), Job::icon_asset_path)
    }
}
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) { app.add_systems(PostUpdate, PlayerSprite::update_sprites); }
}

pub fn plugin() -> PlayerPlugin { PlayerPlugin }
