use std::io;

use bevy::{
    asset::{AssetLoader, AsyncReadExt, ParseAssetPathError},
    prelude::*,
    render::camera::ScalingMode,
};
use serde::Deserialize;
use thiserror::Error;

use crate::cursor::DragSurface;

/// A list of all the supported maps, used to hardcode asset paths.
///
/// TODO: Generate the list dynamically?
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Map {
    TeaP1,
}

impl Map {
    pub fn asset_path(self) -> &'static str {
        match self {
            Map::TeaP1 => "arenas/ultimate/tea/p1.arena.ron",
        }
    }
}

#[derive(Component, Reflect, Clone, Debug, Deserialize)]
pub struct ArenaData {
    pub name: String,
    pub short_name: String,
    /// The FFXIV map ID.
    pub map_id: u32,
    pub background_path: String,
    /// The size of the arena image, in yalms.
    pub size: Vec2,
    /// The in-game coordinates of the center of the arena, in yalms.
    /// The Y-coordinate in stratmat corresponds to the Z-coordinate in FFXIV.
    /// Used for import/export only; internally the origin is always the center.
    pub offset: Vec2,
}

#[derive(Default, Copy, Clone, Debug)]
pub struct ArenaLoader;

#[derive(Error, Debug)]
pub enum ArenaLoadError {
    #[error("Could not load asset file: {0}")]
    IoError(#[from] io::Error),
    #[error("Could not parse asset file: {0}")]
    ParseError(#[from] ron::error::SpannedError),
    #[error("Invalid image path in arena asset: {0}")]
    ImagePathError(#[from] ParseAssetPathError),
}

impl AssetLoader for ArenaLoader {
    type Asset = Arena;
    type Settings = ();
    type Error = ArenaLoadError;

    fn load<'a>(
        &'a self,
        reader: &'a mut bevy::asset::io::Reader,
        _settings: &'a Self::Settings,
        load_context: &'a mut bevy::asset::LoadContext,
    ) -> bevy::utils::BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).await?;
            let data: ArenaData = ron::de::from_bytes(&buf)?;
            let background_path = load_context
                .asset_path()
                .parent()
                .unwrap_or_else(|| "".into())
                .resolve(&data.background_path)?;
            debug!(
                "for arena {}: loading background image: {}",
                data.name, background_path
            );
            Ok(Arena {
                data,
                background_image: load_context.load(background_path),
            })
        })
    }

    fn extensions(&self) -> &[&str] {
        &["arena.ron"]
    }
}

#[derive(Asset, Reflect, Clone, Debug)]
pub struct Arena {
    data: ArenaData,
    background_image: Handle<Image>,
}

/// Marker component for the current arena background.
///
/// Currently only one is allowed at a time.
///
/// TODO: Make more than one allowed at a time.
#[derive(Component, Reflect, Copy, Clone, Debug)]
pub struct ArenaBackground;

/// How big the viewport should be relative to the size of the arena.
const ARENA_VIEWPORT_SCALE: f32 = 1.1;

/// Z-coordinate of the arena background.
///
/// Our default viewport is (-1000.0, 1000.0), so make sure we are ever so slightly inside that.
const ARENA_BACKGROUND_Z: f32 = -999.0;

impl ArenaBackground {
    pub fn handle_events(
        q: Query<(Entity, &Handle<Arena>), With<ArenaBackground>>,
        mut camera_q: Query<&mut OrthographicProjection, With<Camera2d>>,
        arenas: Res<Assets<Arena>>,
        mut evs: EventReader<AssetEvent<Arena>>,
        mut commands: Commands,
    ) {
        for ev in evs.read() {
            match ev {
                AssetEvent::LoadedWithDependencies { id: arena_id }
                | AssetEvent::Modified { id: arena_id } => {
                    for (id, handle) in q.iter().filter(|(_, handle)| handle.id() == *arena_id) {
                        if let Some(arena) = arenas.get(handle) {
                            commands.entity(id).insert((
                                Name::new(format!("Arena Background of {}", arena.data.short_name)),
                                arena.data.clone(),
                                DragSurface,
                                SpriteBundle {
                                    sprite: Sprite {
                                        custom_size: Some(arena.data.size),
                                        ..default()
                                    },
                                    texture: arena.background_image.clone(),
                                    transform: Transform::from_xyz(0.0, 0.0, ARENA_BACKGROUND_Z),
                                    ..default()
                                },
                            ));
                            camera_q.single_mut().scaling_mode = ScalingMode::AutoMin {
                                min_width: arena.data.size.x * ARENA_VIEWPORT_SCALE,
                                min_height: arena.data.size.y * ARENA_VIEWPORT_SCALE,
                            };
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

#[derive(Debug, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct ArenaPlugin;

impl Plugin for ArenaPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Arena>()
            .register_type::<Arena>()
            .register_type::<ArenaData>()
            .init_asset_loader::<ArenaLoader>()
            .add_systems(
                First,
                ArenaBackground::handle_events.run_if(on_event::<AssetEvent<Arena>>()),
            )
            .add_systems(Startup, spawn_tea_p1);
    }
}

fn spawn_tea_p1(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Name::new("Background Loader for Tea P1"),
        ArenaBackground,
        asset_server.load::<Arena>(Map::TeaP1.asset_path()),
    ));
}

pub fn plugin() -> ArenaPlugin {
    ArenaPlugin
}
