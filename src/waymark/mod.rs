//! Waymark support.
//!
//! This module implements support for FFXIV waymarks.
//! Waymarks can be manually manipulated, as well as imported and exported using the format of the Waymark Preset plugin.

use bevy::ecs::query::QuerySingleError;
use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy::window::RequestRedraw;
use bevy_commandify::{command, entity_command};
use bevy_mod_picking::prelude::*;
use bevy_vector_shapes::prelude::*;
use bevy_xpbd_2d::prelude::*;
use enum_iterator::Sequence;
use int_enum::IntEnum;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::arena::Arena;
use crate::cursor::DraggableBundle;
use crate::ecs::AssetCommands;

pub mod window;

/// The diameter, in yalms, of a waymark.
const WAYMARK_SIZE: f32 = 2.4;
/// The scaling to apply to the waymark letter/number image.
const IMAGE_SCALE: f32 = 1.0;
/// The opacity of the fill of a waymark.
const FILL_OPACITY: f32 = 0.22;
/// The opacity of the outer line of a waymark.
const STROKE_OPACITY: f32 = 0.75;
/// The stroke width of the outer line of a waymark.
const STROKE_WIDTH: f32 = 0.05;

/// A waymark preset in the JSON format of the Waymark Preset plugin.
///
/// This type can be directly serialized from/to the Waymark Preset format.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Preset {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "MapID")]
    map_id: u32,

    #[serde(flatten)]
    waymarks: HashMap<Waymark, PresetEntry>,
}

/// A single waymark entry in the Waymark Preset format.
///
/// Coordinates are all in the FFXIV coordinate system, not the Stratmap coordinate system.
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct PresetEntry {
    /// Corresponds to the X axis in Stratmap.
    #[serde(rename = "X")]
    x: f32,
    /// Would be the Z axis in Stratmap and therefore always ignored by us.
    #[serde(rename = "Y")]
    y: f32,
    /// Corresponds to the negative Y axis in Stratmap.
    #[serde(rename = "Z")]
    z: f32,
    /// Numeric ID of the waymark (redundant but important for the plugin).
    #[serde(rename = "ID")]
    id: u8,
    /// Whether the waymark is active. Stratmat simply discards inactive waymarks.
    #[serde(rename = "Active")]
    active: bool,
}

/// A placeable marker for players to reference movements during a fight.
#[repr(u8)]
#[derive(Copy, Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
#[derive(Component, Reflect, Serialize, Deserialize)]
#[derive(IntEnum, Sequence)]
pub enum Waymark {
    A = 0,
    B = 1,
    C = 2,
    D = 3,
    One = 4,
    Two = 5,
    Three = 6,
    Four = 7,
}

impl Waymark {
    /// Produces the asset path for the image with the letter or number of the waymark.
    pub fn asset_path(self) -> &'static str {
        match self {
            Waymark::One => "waymarks/way_1.png",
            Waymark::Two => "waymarks/way_2.png",
            Waymark::Three => "waymarks/way_3.png",
            Waymark::Four => "waymarks/way_4.png",
            Waymark::A => "waymarks/way_a.png",
            Waymark::B => "waymarks/way_b.png",
            Waymark::C => "waymarks/way_c.png",
            Waymark::D => "waymarks/way_d.png",
        }
    }

    /// Retrieves a [Handle] to this image asset with the letter or number of the waymark.
    pub fn asset_handle(self, asset_server: &AssetServer) -> Handle<Image> {
        asset_server.load(self.asset_path())
    }

    /// Produces the fill/stroke colour for this waymark.
    pub fn color(self) -> Color {
        match self {
            Waymark::One | Waymark::A => Color::RED,
            Waymark::Two | Waymark::B => Color::YELLOW,
            Waymark::Three | Waymark::C => Color::CYAN,
            Waymark::Four | Waymark::D => Color::FUCHSIA,
        }
    }

    /// Produces true if this waymark is a circle.
    pub fn is_circle(self) -> bool {
        matches!(self, Waymark::A | Waymark::B | Waymark::C | Waymark::D)
    }

    /// Produces true if this waymark is a square.
    pub fn is_square(self) -> bool {
        matches!(
            self,
            Waymark::One | Waymark::Two | Waymark::Three | Waymark::Four
        )
    }

    /// Produces a name suitable for use as an entity label.
    fn name(self) -> &'static str {
        match self {
            Waymark::A => "Waymark A",
            Waymark::B => "Waymark B",
            Waymark::C => "Waymark C",
            Waymark::D => "Waymark D",
            Waymark::One => "Waymark 1",
            Waymark::Two => "Waymark 2",
            Waymark::Three => "Waymark 3",
            Waymark::Four => "Waymark 4",
        }
    }

    /// Produces a [`PresetEntry`] corresponding to this waymark,
    /// using the provided [`Arena`] center `offset` and the provided [`Transform`].
    pub fn to_entry(self, transform: &Transform, offset: Vec2) -> PresetEntry {
        PresetEntry {
            x: offset.x + transform.translation.x,
            y: 0.0,
            // The entry's Z axis is our negative Y axis.
            z: offset.y - transform.translation.y,
            id: u8::from(self),
            active: true,
        }
    }

    /// Spawns the entities for this waymark.
    ///
    /// The entities include the `Waymark` entity itself as well as the necessary sprite entities
    /// to render it correctly.
    pub fn spawn(self, commands: &mut Commands<'_, '_>) {
        commands.spawn_empty().insert_waymark(self, None);
    }
}

/// [`Command`](bevy::ecs::system::Command) to spawn all of the waymarks specified in the given `preset`.
#[command]
pub fn spawn_waymarks_from_preset(preset: Preset, world: &mut World) {
    for (waymark, entry) in preset.waymarks {
        if entry.active {
            world.spawn_empty().insert_waymark(waymark, Some(entry));
        }
    }
}

/// [`Command`](bevy::ecs::system::Command) to despawn all active waymarks.
#[command]
pub fn despawn_all_waymarks(world: &mut World) {
    let mut query = world.query_filtered::<Entity, &Waymark>();
    let entities = query.iter(world).collect_vec();
    for entity in entities {
        world.entity_mut(entity).despawn_recursive();
    }
    world.send_event(RequestRedraw);
}

#[derive(Bundle)]
struct WaymarkBundle {
    name: Name,
    waymark: Waymark,
    pickable: PickableBundle,
    draggable: DraggableBundle,
    spatial: SpatialBundle,
    collider: Collider,
}

impl WaymarkBundle {
    fn new(waymark: Waymark) -> Self {
        Self {
            name: Name::new(waymark.name()),
            waymark,
            pickable: PickableBundle::default(),
            draggable: DraggableBundle::default(),
            spatial: SpatialBundle::default(),
            collider: if waymark.is_square() {
                Collider::cuboid(WAYMARK_SIZE, WAYMARK_SIZE)
            } else {
                Collider::ball(WAYMARK_SIZE / 2.0)
            },
        }
    }
}

#[derive(Error, Debug)]
enum InsertWaymarkError {
    #[error("Unable to retrieve arena: {0}")]
    Single(#[from] QuerySingleError),
    #[error("Arena data not loaded")]
    NotLoaded(AssetId<Arena>),
}

/// [`EntityCommand`](bevy::ecs::system::EntityCommand) to insert a waymark's entities.
///
/// If a [`PresetEntry`] is provided, it will be used to position the waymark.
#[entity_command]
pub fn insert_waymark(id: Entity, world: &mut World, waymark: Waymark, entry: Option<PresetEntry>) {
    debug!("inserting waymark {:?} on entity {:?}", waymark, id);
    let asset_server = world.resource::<AssetServer>();
    let image = waymark.asset_handle(asset_server);

    let Some(mut entity) = world.get_entity_mut(id) else {
        return;
    };
    entity.insert(WaymarkBundle::new(waymark));

    if let Some(entry) = entry {
        match entity.world_scope(|world| {
            world.resource_scope(|world: &mut World, arenas: Mut<Assets<Arena>>| {
                let mut arena_q = world.query::<&Handle<Arena>>();
                let handle = arena_q.get_single(world)?;
                arenas
                    .get(handle)
                    .map(|arena| arena.offset)
                    .ok_or(InsertWaymarkError::NotLoaded(handle.id()))
            })
        }) {
            Ok(offset) => {
                entity.insert(Transform::from_xyz(
                    entry.x - offset.x,
                    // The entry's Z axis is our negative Y axis.
                    offset.y - entry.z,
                    0.0,
                ));
            }
            Err(InsertWaymarkError::NotLoaded(asset_id)) => {
                // Defer execution of the command until the arena finishes loading.
                debug!("waymark {waymark:?} spawn deferred until arena is loaded");
                world.resource_mut::<AssetCommands<Arena>>().on_load(
                    asset_id,
                    InsertWaymarkEntityCommand {
                        waymark,
                        entry: Some(entry),
                    }
                    .with_entity(id),
                );
                return;
            }
            Err(e) => {
                error!("Unable to insert waymark: {e}");
                return;
            }
        }
    }

    entity.with_children(|parent| {
        parent.spawn((
            Name::new("Waymark Image"),
            SpriteBundle {
                sprite: Sprite {
                    custom_size: Some(Vec2::new(
                        WAYMARK_SIZE * IMAGE_SCALE,
                        WAYMARK_SIZE * IMAGE_SCALE,
                    )),
                    ..default()
                },
                texture: image,
                ..default()
            },
        ));

        let mut spawn_shape = |name, config| {
            if waymark.is_square() {
                parent.spawn((
                    Name::new(name),
                    ShapeBundle::rect(&config, Vec2::new(WAYMARK_SIZE, WAYMARK_SIZE)),
                ));
            } else {
                parent.spawn((
                    Name::new(name),
                    ShapeBundle::circle(&config, WAYMARK_SIZE / 2.0),
                ));
            };
        };

        spawn_shape(
            "Waymark Stroke",
            ShapeConfig {
                color: waymark.color().with_a(STROKE_OPACITY),
                thickness: STROKE_WIDTH,
                hollow: true,
                alpha_mode: AlphaMode::Blend,
                transform: Transform::from_xyz(0.0, 0.0, -0.1),
                ..ShapeConfig::default_2d()
            },
        );

        spawn_shape(
            "Waymark Fill",
            ShapeConfig {
                color: waymark.color().with_a(FILL_OPACITY),
                hollow: false,
                alpha_mode: AlphaMode::Blend,
                transform: Transform::from_xyz(0.0, 0.0, -0.2),
                ..ShapeConfig::default_2d()
            },
        );
    });
}

/// Plugin for waymark support.
#[derive(Default, Copy, Clone, Debug)]
pub struct WaymarkPlugin;

impl Plugin for WaymarkPlugin {
    fn build(&self, _app: &mut App) {}
}
