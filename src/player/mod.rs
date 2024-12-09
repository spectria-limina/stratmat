use avian2d::prelude::Collider;
use bevy::prelude::*;
use job::Job;

use crate::{drag::Draggable, spawner::Spawnable};

pub mod job;
pub mod window;

/// The size of a player icon.
const PLAYER_SPRITE_SIZE: f32 = 2.0;

const PLAYER_COLLIDER_SIZE: f32 = 0.001;

#[derive(Copy, Clone, Hash, PartialEq, Eq, Ord, PartialOrd, Component, Reflect)]
#[require(Draggable)]
#[require(Collider(|| Collider::circle(PLAYER_COLLIDER_SIZE)))]
#[require(PlayerSprite)]
pub struct Player {}

impl Player {}

#[derive(Copy, Default, Clone, Hash, PartialEq, Eq, Ord, PartialOrd, Debug)]
#[derive(Component, Reflect)]
#[require(Sprite(||Sprite{custom_size: Some(Vec2::splat(PLAYER_SPRITE_SIZE)), ..default()}))]
pub struct PlayerSprite {
    pub job: Option<Job>,
}

impl PlayerSprite {
    pub fn update_sprites(
        mut q: Query<(&PlayerSprite, &mut Sprite), Changed<PlayerSprite>>,
        asset_server: Res<AssetServer>,
    ) {
        for (player_sprite, mut sprite) in &mut q {
            sprite.image = asset_server.load(player_sprite.asset_path());
            sprite.custom_size = Some(Vec2::splat(PLAYER_SPRITE_SIZE));
        }
    }

    pub fn asset_path(self) -> &'static str {
        self.job
            .map_or(Job::none_asset_path(), Job::icon_asset_path)
    }
}

impl Spawnable for PlayerSprite {
    const UNIQUE: bool = true;

    fn spawner_name(&self) -> std::borrow::Cow<'static, str> {
        format!("{:#?} Spawner", self.job).into()
    }

    fn texture_handle(&self, asset_server: &AssetServer) -> Handle<Image> {
        asset_server.load(self.asset_path())
    }

    fn insert(&self, entity: &mut EntityCommands) {
        entity.insert((Player {}, *self));
    }
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, PlayerSprite::update_sprites);
    }
}

pub fn plugin() -> PlayerPlugin {
    PlayerPlugin
}
