use bevy::prelude::*;
use bevy::render::camera::ScalingMode;

#[derive(Clone, Resource, Debug)]
pub struct Arena {
    pub name: &'static str,
    pub image_handle: Handle<Image>,
    /// The size of the arena image, in yalms.
    pub size: Vec2,
    /// The in-game coordinates of the center of the arena, in yalms.
    /// The Y-coordinate in stratmat corresponds to the Z-coordinate in FFXIV.
    /// Used for import/export only; internally the origin is always the center.
    pub offset: Vec2,
    /// The FFXIV map ID.
    pub map_id: u32,
}

impl FromWorld for Arena {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.get_resource::<AssetServer>().unwrap();
        let image_handle = asset_server.load("arenas/p9s.jpg");
        Self {
            name: "P9S: Anabaseios: The Ninth Circle (Kokytos)",
            image_handle,
            size: Vec2::new(44.0, 44.0),
            map_id: 937,
            offset: Vec2::new(100.0, 100.0),
        }
    }
}

#[derive(Clone, Component, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct ArenaView {}

#[derive(Debug, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct ArenaPlugin {}

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Arena>().add_systems(
            Startup,
            |mut commands: Commands, arena: Res<Arena>| {
                commands.spawn(Camera2dBundle {
                    projection: OrthographicProjection {
                        near: -1000.0,
                        far: 1000.0,
                        scaling_mode: ScalingMode::AutoMin {
                            min_width: arena.size.x * 1.1,
                            min_height: arena.size.y * 1.1,
                        },
                        ..default()
                    },
                    ..default()
                });
                commands.spawn((
                    ArenaView {},
                    SpriteBundle {
                        sprite: Sprite {
                            custom_size: Some(arena.size),
                            ..default()
                        },
                        texture: arena.image_handle.clone(),
                        ..default()
                    },
                ));
            },
        );
    }
}

pub fn plugin() -> ArenaPlugin {
    ArenaPlugin {}
}
