//! Waymark support.
//!
//! This module implements support for FFXIV waymarks.
//! Waymarks can be manually manipulated, as well as imported and exported using the format of the Waymark Preset plugin.

use avian2d::prelude::*;
#[cfg(feature = "egui")]
use bevy::window::RequestRedraw;
use bevy::{
    color::palettes::css::{FUCHSIA, LIGHT_CYAN, RED, YELLOW},
    ecs::{component::ComponentId, world::DeferredWorld},
    prelude::*,
    utils::HashMap,
};
#[cfg(feature = "egui")]
use bevy_vector_shapes::prelude::*;
use enum_iterator::Sequence;
use int_enum::IntEnum;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use crate::{
    arena::{Arena, GameCoordOffset},
    color::AlphaScale,
    drag::Draggable,
    image::DrawImage,
    shape::{ColliderFromShape, DrawShape, Shape, Stroke},
};

#[cfg(feature = "egui")]
mod window_egui;
pub mod window {
    #[cfg(feature = "egui")]
    pub use super::window_egui::*;
}

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
const WAYMARK_Z: f32 = 100.0;

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
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Component, Reflect)]
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
#[require(Draggable, Collider)]
#[cfg_attr(feature = "egui", require(Visibility))]
#[component(on_add = Self::on_add)]
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
            Waymark::One => "sprites/waymarks/way_1.png",
            Waymark::Two => "sprites/waymarks/way_2.png",
            Waymark::Three => "sprites/waymarks/way_3.png",
            Waymark::Four => "sprites/waymarks/way_4.png",
            Waymark::A => "sprites/waymarks/way_a.png",
            Waymark::B => "sprites/waymarks/way_b.png",
            Waymark::C => "sprites/waymarks/way_c.png",
            Waymark::D => "sprites/waymarks/way_d.png",
        }
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

    pub fn spawn_from_preset(commands: &mut Commands, preset: Preset, parent: Entity) {
        for (waymark, entry) in preset.waymarks {
            if entry.active {
                commands.spawn((waymark, entry)).set_parent(parent);
            }
        }
    }

    pub fn despawn_all(world: &mut World) {
        let mut query = world.query_filtered::<Entity, With<Waymark>>();
        let entities = query.iter(world).collect_vec();
        for entity in entities {
            world.entity_mut(entity).despawn_recursive();
        }
        #[cfg(feature = "egui")]
        world.send_event(RequestRedraw);
    }

    fn on_add(mut world: DeferredWorld, id: Entity, _: ComponentId) {
        world.commands().run_system_cached_with(system, id);

        fn system(
            In(id): In<Entity>,
            q: Query<(&Waymark, Option<&PresetEntry>)>,
            arena_q: Single<Entity, With<Arena>>,
            asset_server: Res<AssetServer>,
            offset: Option<Res<GameCoordOffset>>,
            mut commands: Commands,
        ) {
            let Ok((&waymark, preset_entry)) = q.get(id) else {
                error!(
                    "Waymark doesn't exist on {:#?} right after it was added",
                    id
                );
                return;
            };
            debug!("inserting waymark {waymark:?} on entity {id:?} with preset {preset_entry:?}");

            let mut entity = commands.entity(id);

            if let Some(entry) = preset_entry {
                if let Some(offset) = offset {
                    let (x, y) = (entry.x - offset.x, offset.y - entry.z);
                    debug!("world coords: {:?}", (x, y),);
                    entity.insert(Transform::from_xyz(
                        entry.x - offset.x,
                        // The entry's Z axis is our negative Y axis.
                        offset.y - entry.z,
                        WAYMARK_Z,
                    ));
                } else {
                    error!("Unable to spawn waymark because GameCoordOffset is not available.");
                    return;
                }
            } else {
                entity.insert(Transform::from_xyz(0.0, 0.0, WAYMARK_Z));
            }

            let shape = if waymark.is_square() {
                Shape::Circle(Circle::new(WAYMARK_SIZE / 2.0))
            } else {
                Shape::Rectangle(Rectangle::from_length(WAYMARK_SIZE))
            };

            entity.insert((Name::new(waymark.name()), waymark, shape, ColliderFromShape));
            entity.remove::<PresetEntry>();

            entity.with_children(|parent| {
                #[cfg_attr(not(feature = "egui"), allow(unused_variables))]
                let mut image_child = parent.spawn((
                    Name::new("Waymark Image"),
                    DrawImage::new(
                        waymark.asset_path().into(),
                        Vec2::splat(WAYMARK_SIZE * IMAGE_SCALE),
                    ),
                    AlphaScale::default(),
                ));
                #[cfg(feature = "egui")]
                image_child.insert(Sprite::default());

                parent.spawn((
                    Name::new("Waymark Shape"),
                    shape,
                    DrawShape::new(
                        waymark.color().with_alpha(FILL_OPACITY),
                        Stroke::new(waymark.color().with_alpha(STROKE_OPACITY), STROKE_WIDTH),
                    ),
                    Transform::from_xyz(0.0, 0.0, -0.1),
                ));
            });
        }
    }
}

/// Plugin for waymark support.
#[derive(Default, Copy, Clone, Debug)]
pub struct WaymarkPlugin;

impl Plugin for WaymarkPlugin {
    fn build(&self, _app: &mut App) {}
}

pub fn plugin() -> WaymarkPlugin { WaymarkPlugin }
