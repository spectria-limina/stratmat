use std::f32::consts::PI;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_vector_shapes::{
    painter::ShapeConfig,
    shapes::{DiscBundle, ShapeBundle},
};

/// The specific type of hitbox. Defines several important properties.
#[derive(Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum HitboxKind {
    /// A standard directional enemy hitbox, drawn as 3/4 of a circle with chevrons at the side.
    /// Collision is measured from the edge of the hitbox.
    #[default]
    Directional,
    /// An omnidirectional hitbox, drawn as a full circle. All positionals are always hit against an omni hitbox.
    /// Collision is measured from the edge of the hitbox.
    Omni,
    /// A player hitbox, drawn the same as a directional hitbox, but with point collision only at the center of the hitbox.
    Player,
}

#[derive(Component, Clone, Debug)]
pub struct Hitbox {
    pub kind: HitboxKind,
    pub color: Color,
    pub outer_radius: f32,
    pub inner_radius: f32,
}

/// The default ratio of the inner circle radius to the outer radius
// TODO: This is wrong; it's somewhat accurate for large hitboxes but very wrong for small ones.
const INNER_CIRCLE_DEFAULT_RATIO: f32 = 0.85;
/// The thickness of the outer circle, as a ratio of the outer circle radius.
const OUTER_CIRCLE_THICKNESS_RATIO: f32 = 0.02;
/// The thickness of the outer circle, as a ratio of the inner circle radius.
const INNER_CIRCLE_THICKNESS_RATIO: f32 = 0.01;

impl Hitbox {
    /// Construct a new hitbox. The inner radius is inferred from the outer radius.
    pub fn new(kind: HitboxKind, color: Color, outer_radius: f32) -> Self {
        Self {
            kind,
            color,
            outer_radius,
            inner_radius: 0.85 * outer_radius,
        }
    }

    /// Modify a hitbox's inner
    pub fn with_inner_radius(&mut self, inner_radius: f32) -> &mut Self {
        self.inner_radius = inner_radius;
        self
    }

    /// Returns true if this hitbox is directional, including player hitboxes
    pub fn is_directional(&self) -> bool {
        matches!(self.kind, HitboxKind::Directional | HitboxKind::Player)
    }

    /// Construct a collider for this hitbox
    pub fn collider(&self) -> Collider {
        Collider::circle(if self.kind == HitboxKind::Player {
            0.001 // There's no support for point colliders so use a very small circle.
        } else {
            self.outer_radius
        })
    }
}

impl Default for Hitbox {
    fn default() -> Self { Self::new(default(), bevy::color::palettes::css::SALMON.into(), 10.0) }
}

#[derive(Bundle, Default)]
struct HitboxBundle {
    hitbox: Hitbox,
    transform: Transform,
    visibility: Visibility,
    collider: Collider,
}

pub fn insert_hitbox(entity: &mut EntityCommands, hitbox: Hitbox) {
    entity.insert_if_new((GlobalTransform::default(), InheritedVisibility::default()));
    entity.with_children(|parent| {
        parent
            // Insert the hitbox last so that it's not used after move.
            // Hence, start with an empty spawn.
            .spawn_empty()
            .with_children(|parent| {
                let shape_bundle = |radius, config| {
                    if hitbox.is_directional() {
                        ShapeBundle::arc(&config, radius, -3.0 * PI / 4.0, 3.0 * PI / 4.0)
                    } else {
                        ShapeBundle::circle(&config, radius)
                    }
                };

                parent.spawn(shape_bundle(hitbox.outer_radius, ShapeConfig {
                    color: hitbox.color,
                    thickness: hitbox.outer_radius * OUTER_CIRCLE_THICKNESS_RATIO,
                    hollow: true,
                    ..ShapeConfig::default_2d()
                }));

                parent.spawn(shape_bundle(hitbox.inner_radius, ShapeConfig {
                    color: hitbox.color,
                    thickness: hitbox.inner_radius * INNER_CIRCLE_THICKNESS_RATIO,
                    hollow: true,
                    ..ShapeConfig::default_2d()
                }));
            })
            .insert(HitboxBundle {
                collider: hitbox.collider(),
                hitbox,
                ..default()
            });
    });
}
