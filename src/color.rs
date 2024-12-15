use bevy::prelude::{Alpha as _, *};
#[cfg(feature = "egui")]
use bevy_vector_shapes::prelude::*;
use serde::{Deserialize, Serialize};

/// Configurable alpha value.
///
/// With the [ColorPlugin], will automatically update other components to make use of the value.
/// Inherited through the hierarchy onto [ComputedAlpha].
#[derive(Component, Copy, Clone, Debug)]
#[derive(PartialOrd, PartialEq, Reflect, Serialize, Deserialize)]
#[require(ComputedAlpha)]
pub struct AlphaScale(pub f32);

impl Default for AlphaScale {
    fn default() -> Self { Self(1.0) }
}

/// Computed alpha value.
///
/// Computed as the parent's alpha value times this entity's alpha value.
/// This means it may not be suitable for all blending modes.
#[derive(Component, Copy, Clone, Debug)]
#[derive(PartialOrd, PartialEq, Reflect, Serialize, Deserialize)]
pub struct ComputedAlpha(pub f32);

impl Default for ComputedAlpha {
    fn default() -> Self { Self(1.0) }
}

/// Plugin to register HasColor for trait query support.
pub struct ColorPlugin;

impl Plugin for ColorPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<AlphaScale>()
            .register_type::<ComputedAlpha>()
            .add_systems(PostUpdate, propagate_alpha);
        #[cfg(feature = "egui")]
        app.add_systems(
            PostUpdate,
            (update_sprite_alpha, update_shape_alpha).after(propagate_alpha),
        );
    }
}

#[cfg(feature = "egui")]
pub fn update_sprite_alpha(mut q: Query<(&mut Sprite, &ComputedAlpha)>) {
    for (mut sprite, ComputedAlpha(alpha)) in &mut q {
        sprite.color.set_alpha(*alpha)
    }
}

#[cfg(feature = "egui")]
pub fn update_shape_alpha(mut q: Query<(&mut ShapeFill, &ComputedAlpha)>) {
    for (mut shape, ComputedAlpha(alpha)) in &mut q {
        shape.color.set_alpha(*alpha)
    }
}

// These two functions taken from Bevy. They're under the Apache license.
// I'll fix the copyright properly later.
#[allow(clippy::type_complexity)]
fn propagate_alpha(
    changed: Query<
        (Entity, &AlphaScale, Option<&Parent>, Option<&Children>),
        (With<ComputedAlpha>, Changed<AlphaScale>),
    >,
    mut alpha_query: Query<(&AlphaScale, &mut ComputedAlpha)>,
    children_query: Query<&Children, (With<AlphaScale>, With<ComputedAlpha>)>,
) {
    for (entity, AlphaScale(alpha), parent, children) in &changed {
        let new_alpha = alpha
            * parent
                .and_then(|p| alpha_query.get(p.get()).ok())
                .map_or(1.0, |(_, ComputedAlpha(a))| *a);
        let (_, mut computed_alpha) = alpha_query
            .get_mut(entity)
            .expect("With<ComputedAlpha> ensures this query will return a value");

        // Only update the visibility if it has changed.
        // This will also prevent the visibility from propagating multiple times in the same frame
        // if this entity's visibility has been updated recursively by its parent.
        // This is comparing floating point for equality but it's fine.
        // Worst case we will spuriously re-change it.
        if new_alpha != computed_alpha.0 {
            computed_alpha.0 = new_alpha;

            // Recursively update the visibility of each child.
            for &child in children.into_iter().flatten() {
                let _ = propagate_recursive(new_alpha, child, &mut alpha_query, &children_query);
            }
        }
    }
}

fn propagate_recursive(
    parent_alpha: f32,
    entity: Entity,
    alpha_query: &mut Query<(&AlphaScale, &mut ComputedAlpha)>,
    children_query: &Query<&Children, (With<AlphaScale>, With<ComputedAlpha>)>,
    // BLOCKED: https://github.com/rust-lang/rust/issues/31436
    // We use a result here to use the `?` operator. Ideally we'd use a try block instead
) -> Result<(), ()> {
    // Get the visibility components for the current entity.
    // If the entity does not have the required components, just return early.
    let (alpha, mut computed_alpha) = alpha_query.get_mut(entity).map_err(drop)?;

    let new_alpha = alpha.0 * parent_alpha;

    // Only update the visibility if it has changed.
    if computed_alpha.0 != new_alpha {
        computed_alpha.0 = new_alpha;

        // Recursively update the visibility of each child.
        for &child in children_query.get(entity).ok().into_iter().flatten() {
            let _ = propagate_recursive(new_alpha, child, alpha_query, children_query);
        }
    }

    Ok(())
}

/// Produces a new plugin.
pub fn plugin() -> ColorPlugin { ColorPlugin }
