use std::f32::consts::{PI, SQRT_2};

use avian2d::prelude::*;
use bevy::{
    ecs::{component::ComponentId, world::DeferredWorld},
    prelude::*,
};
use bevy_egui::{egui, EguiContexts};
use bevy_vector_shapes::shapes::LineBundle;
#[cfg(feature = "egui")]
use bevy_vector_shapes::{
    painter::ShapeConfig,
    shapes::{DiscBundle, ShapeBundle},
};

#[cfg(feature = "egui")]
use crate::ui::widget::{widget, InitWidget, WidgetCtx};
use crate::ui::{menu::TopMenu, UiSortKey};

/// The specific type of hitbox. Defines several important properties.
#[derive(Default, Reflect, Copy, Clone, Debug)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HitboxKind {
    /// A standard directional enemy hitbox, drawn as 3/4 of a circle with chevrons at the side.
    #[default]
    Directional,
    /// An omnidirectional hitbox, drawn as a full circle. All positionals are always hit against an omni hitbox.
    Omni,
}

#[derive(Component, Reflect, Clone, Debug)]
#[cfg_attr(feature = "egui", require(Visibility))]
#[require(Transform)]
#[component(on_add = Self::on_add)]
pub struct Hitbox {
    pub kind: HitboxKind,
    pub color: Color,
    pub outer_radius: f32,
    pub inner_radius: f32,
}

/// The default ratio of the inner circle radius to the outer radius
// TODO: This is wrong; it's somewhat accurate for large hitboxes but very wrong for small ones.
const INNER_CIRCLE_DEFAULT_RATIO: f32 = 0.83;
/// The thickness of the outer circle, as a ratio of the outer circle radius.
const OUTER_CIRCLE_THICKNESS_RATIO: f32 = 0.02;
/// The thickness of the rear portion of the outer circle, as a ratio of the outer circle radius.
const OUTER_CIRCLE_REAR_THICKNESS_RATIO: f32 = 0.006;
/// The lightness scaling applied to the rear portion of the outer circle.
const OUTER_CIRCLE_REAR_LIGHTNESS_SCALE: f32 = 0.65;
/// The thickness of the outer circle, as a ratio of the inner circle radius.
const INNER_CIRCLE_THICKNESS_RATIO: f32 = 0.01;
/// The range of typical melee weaponskills.
const MAX_MELEE_RANGE: f32 = 3.0;
/// Lightness scaling factor to use when drawing the max melee radius.
const MELEE_RANGE_LIGHTNESS_SCALE: f32 = 0.60;
/// Alpha to use when drawing the max melee radius.
const MELEE_RANGE_ALPHA_SCALE: f32 = 0.25;
/// Line thickness for the melee positional lines, as a ratio to the outer radius.
const MELEE_LINE_THICKNESS_RATIO: f32 = 0.004;
/// Lightness scaling factor to use when drawing the melee positional lines.
const MELEE_LINE_LIGHTNESS_SCALE: f32 = 0.65;
/// Alpha to use when drawing the max melee radius.
const MELEE_LINE_ALPHA_SCALE: f32 = 1.0;

impl Default for Hitbox {
    fn default() -> Self { Self::new(default(), bevy::color::palettes::css::SALMON.into(), 5.0) }
}

impl Hitbox {
    /// Construct a new hitbox. The inner radius is inferred from the outer radius.
    pub fn new(kind: HitboxKind, color: Color, outer_radius: f32) -> Self {
        Self {
            kind,
            color,
            outer_radius,
            inner_radius: INNER_CIRCLE_DEFAULT_RATIO * outer_radius,
        }
    }

    /// Modify a hitbox's inner
    pub fn with_inner_radius(&mut self, inner_radius: f32) -> &mut Self {
        self.inner_radius = inner_radius;
        self
    }

    /// Returns true if this hitbox is directional, including player hitboxes
    pub fn is_directional(&self) -> bool { self.kind == HitboxKind::Directional }

    /// Construct a collider for this hitbox
    pub fn collider(&self) -> Collider { Collider::circle(self.outer_radius) }

    fn on_add(mut world: DeferredWorld, id: Entity, _: ComponentId) {
        let hitbox = world.get::<Hitbox>(id).unwrap().clone();

        #[cfg(feature = "egui")]
        world
            .commands()
            .entity(id)
            .insert_if_new(hitbox.collider())
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

                // Draw an extra tiny little arc just for illustrative/clicking purposes.
                if hitbox.is_directional() {
                    let mut rear_color = Laba::from(hitbox.color);
                    rear_color.lightness *= OUTER_CIRCLE_REAR_LIGHTNESS_SCALE;

                    parent.spawn(ShapeBundle::arc(
                        &ShapeConfig {
                            color: rear_color.into(),
                            thickness: hitbox.outer_radius * OUTER_CIRCLE_REAR_THICKNESS_RATIO,
                            hollow: true,
                            ..ShapeConfig::default_2d()
                        },
                        hitbox.outer_radius,
                        // BVS only draws arcs in a positive direction.
                        // This corresponds to the missing parts of the arcs above.
                        3.0 * PI / 4.0,
                        5.0 * PI / 4.0,
                    ));
                }
            });
        #[cfg(feature = "dom")]
        todo!();
    }

    fn add_max_melee(q: Query<(Entity, &Hitbox), Without<MaxMelee>>, mut commands: Commands) {
        for (id, hitbox) in &q {
            let mut fill_color = Laba::from(hitbox.color);
            fill_color.lightness *= MELEE_RANGE_LIGHTNESS_SCALE;
            fill_color.alpha *= MELEE_RANGE_ALPHA_SCALE;

            let mut line_color = Laba::from(hitbox.color);
            line_color.lightness *= MELEE_LINE_LIGHTNESS_SCALE;
            line_color.alpha *= MELEE_LINE_ALPHA_SCALE;

            let radius = hitbox.outer_radius + MAX_MELEE_RANGE;

            commands
                .entity(id)
                .insert(MaxMelee)
                .with_children(|parent| {
                    parent.spawn((
                        ShapeBundle::circle(
                            &ShapeConfig {
                                color: fill_color.into(),
                                transform: Transform::from_xyz(0.0, 0.0, 0.01),
                                ..ShapeConfig::default_2d()
                            },
                            radius,
                        ),
                        MaxMelee,
                    ));
                    // Draw melee positional lines.
                    if hitbox.is_directional() {
                        // We need the axis-aligned coords of the points on the circle at 45, 135, etc. degrees.
                        let coord = radius / SQRT_2;
                        parent.spawn((
                            ShapeBundle::line(
                                &ShapeConfig {
                                    color: line_color.into(),
                                    thickness: MELEE_LINE_THICKNESS_RATIO * radius,
                                    ..ShapeConfig::default_2d()
                                },
                                Vec3::new(coord, coord, 0.0),
                                Vec3::new(-coord, -coord, 0.0),
                            ),
                            MaxMelee,
                        ));
                        parent.spawn((
                            ShapeBundle::line(
                                &ShapeConfig {
                                    color: line_color.into(),
                                    thickness: MELEE_LINE_THICKNESS_RATIO * hitbox.outer_radius,
                                    ..ShapeConfig::default_2d()
                                },
                                Vec3::new(coord, -coord, 0.0),
                                Vec3::new(-coord, coord, 0.0),
                            ),
                            MaxMelee,
                        ));
                    }
                });
        }
    }

    fn remove_max_melee(q: Query<(Entity, Has<Hitbox>), With<MaxMelee>>, mut commands: Commands) {
        for (id, is_hitbox) in &q {
            if is_hitbox {
                commands.entity(id).remove::<MaxMelee>();
            } else {
                commands.entity(id).remove_parent().despawn();
            }
        }
    }
}

#[derive(Component, Reflect, Default, Copy, Clone, Debug)]
/// Marker struct for melee range indicators.
///
/// For efficiency, this is used to mark both the entities used to draw the melee range
/// and also the parent hitbox. They are distinguished by the presence/absence of [`Hitbox`].
/// This lets us spawn shadows on newly-created entities without a child query.
pub struct MaxMelee;

/// Global setting (with widget) for whether to show max melee radii.
// TODO: Make this not global?
#[derive(Component, Reflect, Default, Copy, Clone, Debug)]
#[cfg_attr(feature = "egui", require(InitWidget(|| widget!())))]
pub struct MeleeRangeToggle {
    pub enabled: bool,
}

impl MeleeRangeToggle {
    #[cfg(feature = "egui")]
    pub fn show(
        WidgetCtx { ns: _ns, id, ui }: WidgetCtx,
        mut toggle: Query<&mut MeleeRangeToggle>,
        mut ctx: EguiContexts,
    ) {
        ui.menu_button("Settings", |ui| {
            ui.checkbox(&mut toggle.get_mut(id).unwrap().enabled, "Show Max Melee");
        });
    }
}

/// Plugin for hitbox support
#[derive(Default, Copy, Clone, Debug)]
pub struct HitboxPlugin {}

impl Plugin for HitboxPlugin {
    fn build(&self, app: &mut App) {
        // TODO: Make this not depend on feature flags.
        #[cfg(feature = "egui")]
        app.add_systems(
            Startup,
            |top: Single<Entity, With<TopMenu>>, mut commands: Commands| {
                commands.entity(*top).with_child((
                    MeleeRangeToggle::default(),
                    UiSortKey(1000),
                    Name::new("Melee Range Toggle"),
                ));
            },
        );
        #[cfg(not(feature = "egui"))]
        app.world_mut().spawn(MeleeRangeToggle::default());
        app.add_systems(
            PostUpdate,
            (
                Hitbox::add_max_melee.run_if(|toggle: Single<&MeleeRangeToggle>| toggle.enabled),
                Hitbox::remove_max_melee
                    .run_if(|toggle: Single<&MeleeRangeToggle>| !toggle.enabled),
            ),
        );
    }
}

pub fn plugin() -> HitboxPlugin { default() }
