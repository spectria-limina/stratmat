use avian2d::prelude::{Collider, PhysicsSet};
use bevy::prelude::*;
use i_cant_believe_its_not_bsn::WithChild;
use serde::{Deserialize, Serialize};

use crate::color::AlphaScale;

#[cfg(feature = "egui")]
mod egui;
#[cfg(feature = "egui")]
pub use egui::*;

#[derive(Copy, Clone, Debug, Component, Reflect, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Shape {
    Circle(Circle),
    Rectangle(Rectangle),
}

impl From<Shape> for Collider {
    fn from(value: Shape) -> Self {
        match value {
            Shape::Circle(Circle { radius }) => Collider::circle(radius),
            Shape::Rectangle(rect) => Collider::rectangle(rect.size().x, rect.size().y),
        }
    }
}

#[derive(Copy, Clone, Debug, Component, Default)]
#[derive(Reflect, Serialize, Deserialize)]
#[require(Shape(|| ->Shape{ panic!("ShapeCollider must have a Shape")}))]
pub struct ColliderFromShape;

impl ColliderFromShape {
    pub fn update_colliders(
        q: Query<(Entity, &Shape), (Changed<Shape>, With<ColliderFromShape>)>,
        mut commands: Commands,
    ) {
        for (id, shape) in &q {
            commands.entity(id).insert(Collider::from(*shape));
        }
    }
}

#[derive(Copy, Clone, Debug, Component, Reflect, Serialize, Deserialize)]
#[require(AlphaScale, Transform, Visibility)]
#[require(Shape(||->Shape{ panic!("ShapeDraw must have a Shape")}))]
#[cfg_attr(feature = "egui", require(WithChild<ShapeFill>, WithChild<ShapeStroke>))]
pub struct DrawShape {
    fill: Option<Color>,
    stroke: Option<Stroke>,
}

impl DrawShape {
    pub fn new(fill: Color, stroke: Stroke) -> Self {
        Self {
            fill: Some(fill),
            stroke: Some(stroke),
        }
    }
    pub fn new_fill(fill: Color) -> Self {
        Self {
            fill: Some(fill),
            stroke: None,
        }
    }
    pub fn new_stroke(stroke: Stroke) -> Self {
        Self {
            fill: None,
            stroke: Some(stroke),
        }
    }
}

#[derive(Copy, Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct Stroke {
    color: Color,
    thickness: f32,
}

impl Stroke {
    pub fn new(color: Color, thickness: f32) -> Self { Self { color, thickness } }
}

pub struct ShapePlugin;

impl Plugin for ShapePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            ColliderFromShape::update_colliders.before(PhysicsSet::Prepare),
        );
        #[cfg(feature = "egui")]
        app.add_systems(PostUpdate, DrawShape::update_vector_shapes);
    }
}

pub fn plugin() -> ShapePlugin { ShapePlugin }
