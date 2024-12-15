use bevy_vector_shapes::prelude::*;
use itertools::Itertools;

use super::*;

#[derive(Copy, Clone, Debug, Default, Component)]
#[derive(Reflect, Serialize, Deserialize)]
#[require(AlphaScale, Transform(|| Transform::from_xyz(0.0, 0.0, -0.1)), Visibility)]
pub struct ShapeFill;
#[derive(Copy, Clone, Debug, Default, Component)]
#[derive(Reflect, Serialize, Deserialize)]
#[require(AlphaScale, Transform, Visibility)]
pub struct ShapeStroke;

type AllBvsComps = (ShapeMaterial, ShapeFill, DiscComponent, RectangleComponent);

impl DrawShape {
    pub fn update_vector_shapes(
        q: Query<(&Shape, &DrawShape, &Children), Or<(Changed<Shape>, Changed<DrawShape>)>>,
        fill_q: Query<Entity, With<ShapeFill>>,
        stroke_q: Query<Entity, With<ShapeStroke>>,
        mut commands: Commands,
    ) {
        for (shape, draw, children) in &q {
            let bvs_material = ShapeMaterial::default();

            let fill_id = fill_q.iter_many(children.iter()).exactly_one().unwrap();
            let mut fill_entity = commands.entity(fill_id);
            if let Some(color) = draw.fill {
                let bvs_fill = bevy_vector_shapes::shapes::ShapeFill {
                    color,
                    ty: FillType::Fill,
                };
                fill_entity.insert((bvs_material.clone(), bvs_fill, AlphaScale(color.alpha())));
                match shape {
                    Shape::Circle(Circle { radius }) => {
                        fill_entity.insert(DiscComponent {
                            radius: *radius,
                            ..default()
                        });
                    }
                    Shape::Rectangle(rect) => {
                        fill_entity.insert(RectangleComponent {
                            size: rect.size(),
                            ..default()
                        });
                    }
                }
            } else {
                fill_entity.remove::<AllBvsComps>();
            }

            let stroke_id = stroke_q.iter_many(children.iter()).exactly_one().unwrap();
            let mut stroke_entity = commands.entity(stroke_id);
            if let Some(stroke) = draw.stroke {
                let bvs_fill = bevy_vector_shapes::shapes::ShapeFill {
                    color: stroke.color,
                    ty: FillType::Stroke(stroke.thickness, ThicknessType::World),
                };
                stroke_entity.insert((bvs_material, bvs_fill, AlphaScale(stroke.color.alpha())));
                match shape {
                    Shape::Circle(Circle { radius }) => {
                        stroke_entity.insert(DiscComponent {
                            radius: *radius,
                            ..default()
                        });
                    }
                    Shape::Rectangle(rect) => {
                        stroke_entity.insert(RectangleComponent {
                            size: rect.size(),
                            ..default()
                        });
                    }
                }
            } else {
                stroke_entity.remove::<AllBvsComps>();
            }
        }
    }
}
