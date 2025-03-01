use std::f32::consts::PI;

use avian2d::prelude::*;
use bevy::{
    ecs::{component::ComponentId, world::DeferredWorld},
    prelude::*,
};
use bevy_egui::{egui, EguiContexts};
#[cfg(feature = "egui")]
use bevy_vector_shapes::{
    painter::ShapeConfig,
    shapes::{DiscBundle, ShapeBundle},
};

use crate::egui::menu::TopMenu;
#[cfg(feature = "egui")]
use crate::egui::widget::{widget, InitWidget, WidgetCtx};

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
/// The thickness of the outer circle, as a ratio of the inner circle radius.
const INNER_CIRCLE_THICKNESS_RATIO: f32 = 0.01;
/// The range of typical melee weaponskills.
const MAX_MELEE_RANGE: f32 = 3.0;
/// Lightness scaling factor to use when drawing the max melee radius.
const MAX_MELEE_LIGHTNESS_SCALING: f32 = 0.60;
/// Alpha to use when drawing the max melee radius.
const MAX_MELEE_ALPHA_SCALING: f32 = 0.25;

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
            });
        #[cfg(feature = "dom")]
        todo!();
    }

    fn add_max_melee(q: Query<(Entity, &Hitbox), Without<MaxMelee>>, mut commands: Commands) {
        for (id, hitbox) in &q {
            let mut color = Laba::from(hitbox.color);
            color.lightness *= MAX_MELEE_LIGHTNESS_SCALING;
            color.alpha *= MAX_MELEE_ALPHA_SCALING;
            commands.entity(id).insert(MaxMelee).with_child((
                ShapeBundle::circle(
                    &ShapeConfig {
                        color: dbg!(color.into()),
                        ..ShapeConfig::default_2d()
                    },
                    hitbox.outer_radius + MAX_MELEE_RANGE,
                ),
                MaxMelee,
            ));
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

#[derive(Component, Default, Copy, Clone, Debug)]
/// Marker struct for max melee radii.
///
/// For efficiency, this is used to mark both the actual max melee shadow entity
/// and the parent hitbox.
/// This lets us spawn shadows on newly-created entities without a child query.
pub struct MaxMelee;

/// Global setting (with widget) for whether to show max melee radii.
// TODO: Make this not global?
#[derive(Component, Default, Copy, Clone, Debug)]
#[cfg_attr(feature = "egui", require(InitWidget(|| widget!())))]
pub struct MaxMeleeToggle {
    pub enabled: bool,
}

impl MaxMeleeToggle {
    #[cfg(feature = "egui")]
    pub fn show(
        WidgetCtx { ns: _ns, id, ui }: WidgetCtx,
        mut toggle: Query<&mut MaxMeleeToggle>,
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
                commands.entity(*top).with_child(MaxMeleeToggle::default());
            },
        );
        #[cfg(not(feature = "egui"))]
        app.world_mut().spawn(MaxMeleeToggle::default());
        app.add_systems(
            PostUpdate,
            (
                Hitbox::add_max_melee.run_if(|toggle: Single<&MaxMeleeToggle>| toggle.enabled),
                Hitbox::remove_max_melee.run_if(|toggle: Single<&MaxMeleeToggle>| !toggle.enabled),
            ),
        );
    }
}

pub fn plugin() -> HitboxPlugin { default() }
