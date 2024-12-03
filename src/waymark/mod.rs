//! Waymark support.
//!
//! This module implements support for FFXIV waymarks.
//! Waymarks can be manually manipulated, as well as imported and exported using the format of the Waymark Preset plugin.

use avian2d::prelude::*;
use bevy::color::palettes::css::{FUCHSIA, LIGHT_CYAN, RED, YELLOW};
use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy::window::RequestRedraw;
use bevy_vector_shapes::prelude::*;
use enum_iterator::Sequence;
use int_enum::IntEnum;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::arena::{Arena, ArenaBackground};
use crate::color::AlphaScale;
use crate::cursor::make_draggable_world;
use crate::ecs::AssetCommandsExt;
use crate::spawner::Spawnable;

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
#[require(Transform, Visibility, Collider, CollidingEntities, AlphaScale)]
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
            Waymark::One | Waymark::A => RED.into(),
            Waymark::Two | Waymark::B => YELLOW.into(),
            Waymark::Three | Waymark::C => LIGHT_CYAN.into(),
            Waymark::Four | Waymark::D => FUCHSIA.into(),
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
    pub fn name(self) -> &'static str {
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
        let mut entity = commands.spawn_empty();
        entity.queue(insert_waymark(self, None));
    }

    pub fn spawn_from_preset(commands: &mut Commands, preset: Preset) {
        for (waymark, entry) in preset.waymarks {
            if entry.active {
                let mut entity = commands.spawn_empty();
                entity.queue(insert_waymark(waymark, Some(entry)));
            }
        }
    }

    pub fn despawn_all(world: &mut World) {
        let mut query = world.query_filtered::<Entity, With<Waymark>>();
        let entities = query.iter(world).collect_vec();
        for entity in entities {
            world.entity_mut(entity).despawn_recursive();
        }
        world.send_event(RequestRedraw);
    }
}

struct InsertWaymark {
    waymark: Waymark,
    entry: Option<PresetEntry>,
}

impl EntityCommand for InsertWaymark {
    fn apply(self, id: Entity, world: &mut World) {
        let InsertWaymark { waymark, entry } = self;
        debug!("inserting waymark {waymark:?} on entity {id:?} with preset {entry:?}",);
        let asset_server = world.resource::<AssetServer>();
        let image = waymark.asset_handle(asset_server);

        if let Some(entry) = entry {
            let mut arena_q = world.query::<&ArenaBackground>();
            match arena_q.get_single(world) {
                Ok(arena) => {
                    world.run_system_when_asset_loaded_with(
                        arena.handle.id(),
                        set_position_from_preset,
                        (id, entry, arena.handle.clone()),
                    );
                }
                Err(e) => {
                    error!("Unable to position waymark by preset because there is no arena: {e}");
                }
            }
        }

        let Ok(mut entity) = world.get_entity_mut(id) else {
            return;
        };
        entity.insert((
            Name::new(waymark.name()),
            waymark,
            if waymark.is_square() {
                Collider::rectangle(WAYMARK_SIZE, WAYMARK_SIZE)
            } else {
                Collider::circle(WAYMARK_SIZE / 2.0)
            },
        ));
        make_draggable_world(&mut entity);

        entity.with_children(|parent| {
            parent.spawn((
                Name::new("Waymark Image"),
                Sprite {
                    image,
                    custom_size: Some(Vec2::new(
                        WAYMARK_SIZE * IMAGE_SCALE,
                        WAYMARK_SIZE * IMAGE_SCALE,
                    )),
                    ..default()
                },
                AlphaScale::default(),
            ));

            let mut spawn_shape = |name, alpha, config| {
                if waymark.is_square() {
                    parent.spawn((
                        Name::new(name),
                        AlphaScale(alpha),
                        ShapeBundle::rect(&config, Vec2::new(WAYMARK_SIZE, WAYMARK_SIZE)),
                    ));
                } else {
                    parent.spawn((
                        Name::new(name),
                        AlphaScale(alpha),
                        ShapeBundle::circle(&config, WAYMARK_SIZE / 2.0),
                    ));
                };
            };

            spawn_shape(
                "Waymark Stroke",
                STROKE_OPACITY,
                ShapeConfig {
                    color: waymark.color(),
                    thickness: STROKE_WIDTH,
                    hollow: true,
                    alpha_mode: AlphaMode::Blend.into(),
                    transform: Transform::from_xyz(0.0, 0.0, -0.1),
                    ..ShapeConfig::default_2d()
                },
            );

            spawn_shape(
                "Waymark Fill",
                FILL_OPACITY,
                ShapeConfig {
                    color: waymark.color(),
                    hollow: false,
                    alpha_mode: AlphaMode::Blend.into(),
                    transform: Transform::from_xyz(0.0, 0.0, -0.2),
                    ..ShapeConfig::default_2d()
                },
            );
        });
    }
}

// Call this only if we are positive the asset indicated by the handle exists.
fn set_position_from_preset(
    In((id, entry, handle)): In<(Entity, PresetEntry, Handle<Arena>)>,
    world: &mut World,
) {
    let offset = world
        .resource::<Assets<Arena>>()
        .get(&handle)
        .unwrap()
        .offset;
    let Ok(mut entity) = world.get_entity_mut(id) else {
        return;
    };
    let (x, y) = (entry.x - offset.x, offset.y - entry.z);
    debug!(
        "arena loaded, repositioning waymark {} to {:?}",
        entry.id,
        (x, y),
    );
    entity.insert(Transform::from_xyz(
        entry.x - offset.x,
        // The entry's Z axis is our negative Y axis.
        offset.y - entry.z,
        0.0,
    ));
}

//
/// If a [`PresetEntry`] is provided, it will be used to position the waymark.
pub fn insert_waymark(waymark: Waymark, entry: Option<PresetEntry>) -> impl EntityCommand {
    InsertWaymark { waymark, entry }
}

impl Spawnable for Waymark {
    const UNIQUE: bool = true;

    fn spawner_name(&self) -> std::borrow::Cow<'static, str> {
        self.name().into()
    }

    fn texture_handle(&self, asset_server: &AssetServer) -> Handle<Image> {
        self.asset_handle(asset_server)
    }

    fn insert(&self, entity: &mut bevy::ecs::system::EntityCommands) {
        entity.queue(insert_waymark(*self, None));
    }
}

/// Plugin for waymark support.
#[derive(Default, Copy, Clone, Debug)]
pub struct WaymarkPlugin;

impl Plugin for WaymarkPlugin {
    fn build(&self, _app: &mut App) {}
}
