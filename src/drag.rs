//! Utilities for working with cursor manipulation.

use std::fmt::Debug;

use avian2d::prelude::*;
use bevy::ecs::component::ComponentId;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;

use crate::color::AlphaScale;
use crate::ecs::{EntityExts, EntityExtsOf};
use crate::Layer;

/// The factor to apply to a sprite's alpha channel when it is dragged out of bounds.
const OOB_ALPHA_FACTOR: f32 = 0.1;

/// Callback to add the update collision mask and add the [`Dragged`] component to newly-dragged entities.
pub fn on_drag_start(event: Trigger<Pointer<DragStart>>, mut commands: Commands) {
    let id = event.entity();
    debug!("dragging {id:?}");
    commands.run_system_cached_with(start_drag, id);
}

/// Implementation of [`on_drag_start`], factored out to allow it to be invoked by spawner logic.
pub fn start_drag(In(id): In<Entity>, world: &mut World) {
    debug!("starting drag on {id:?}");
    let Ok(mut entity) = world.get_entity_mut(id) else {
        debug!("but it couldn't be fetched");
        return;
    };
    if !entity.contains::<Draggable>() {
        debug!("but it isn't draggable");
        return;
    }
    if let Some(mut layers) = entity.get_mut::<CollisionLayers>() {
        layers.memberships.add(Layer::Dragged);
        layers.filters.add(Layer::DragSurface);
    } else {
        entity.insert(CollisionLayers::new([Layer::Dragged], [Layer::DragSurface]));
    }
    entity.insert(Dragged);
}

/// Callback to allow dragging the listener entity around.
///
/// It converts the cursor delta into world coordinates and applies the resulting delta to
/// the [Transform] of the listener entity (not the target entity).
///
/// Will panic if there is not exactly one camera.
pub fn on_drag(
    event: Trigger<Pointer<Drag>>,
    mut q: Query<&mut Transform>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
) {
    trace!("drag_listener");
    let Ok(mut transform) = q.get_mut(event.entity()) else {
        return;
    };
    let (camera, camera_transform) = camera_q.single();

    let new_pos_viewport = event.pointer_location.position;
    let old_pos_viewport = new_pos_viewport - event.delta;
    let new_pos_world = camera
        .viewport_to_world_2d(camera_transform, new_pos_viewport)
        .expect("unable to map cursor position to world coordinates");
    let old_pos_world = camera
        .viewport_to_world_2d(camera_transform, old_pos_viewport)
        .expect("unable to map cursor position to world coordinates");
    let delta_world = new_pos_world - old_pos_world;
    debug!("updating dragged entity position: old_vp: {old_pos_viewport}, new_vp: {new_pos_viewport}, old_world: {}, delta_world: {delta_world}", transform.translation);
    transform.translation += delta_world.extend(0.0);
}

fn drag_update_oob(
    q: Query<(Entity, &CollidingEntities), With<Dragged>>,
    surface_q: Query<&CollisionLayers>,
    mut commands: Commands,
) {
    for (id, collisions) in &q {
        let mut on_surface = false;
        for &surface_id in collisions.iter() {
            if let Ok(layers) = surface_q.get(surface_id) {
                if layers.memberships.has_all(Layer::DragSurface) {
                    on_surface = true;
                    break;
                }
            }
        }

        if on_surface {
            commands.entity(id).remove::<OutOfBounds>();
        } else {
            commands.entity(id).insert(OutOfBounds);
        }
    }
}

/// When the listener entity is dropped [`OutOfBounds`], despawn it and its children, otherwise undoes [`on_drag_start`].
pub fn on_drag_end(
    event: Trigger<Pointer<DragEnd>>,
    mut q: Query<(&mut CollisionLayers, Has<OutOfBounds>)>,
    mut commands: Commands,
) {
    let id = event.entity();
    debug!("ending drag on {id:?}");
    let Ok((mut layers, oob)) = q.get_mut(id) else {
        debug!("but it doesn't exist");
        return;
    };
    if oob {
        debug!("{id:?} dropped out of bounds, despawning");
        commands.entity(id).despawn_recursive();
    } else {
        layers.memberships.remove(Layer::Dragged);
        layers.filters.remove(Layer::DragSurface);
        commands.entity(id).remove::<Dragged>();
    }
}

#[derive(Component, Copy, Clone, Default, Debug)]
#[require(Collider, CollidingEntities, Transform, AlphaScale)]
#[component(on_add = Draggable::add_observers)]
#[component(on_remove = Draggable::remove_observers)]
/// Marker component for draggable entities.
///
/// Will automatically add the necessary hooks when added to an entity.
pub struct Draggable;

/// Marker component for entities currently being dragged.
#[derive(Component, Copy, Clone, Default, Debug)]
#[component(storage = "SparseSet")]
pub struct Dragged;

/// Marker component for out-of-bounds entities.
///
/// When added or removed from an entity, the entity and all its children will have their
/// alpha scaled to make them appear faint while out of bounds.
#[derive(Component, Copy, Clone, Default, Debug)]
#[component(storage = "SparseSet")]
pub struct OutOfBounds;

impl Draggable {
    pub fn add_observers(mut world: DeferredWorld, id: Entity, _: ComponentId) {
        debug!("Adding drag hooks to {id:?}");
        let mut commands = world.commands();
        let mut entity = commands.entity(id);
        let mut of = entity.on::<Self>();
        of.observe(on_drag_start);
        of.observe(on_drag);
        of.observe(on_drag_end);
    }

    pub fn remove_observers(mut world: DeferredWorld, id: Entity, _: ComponentId) {
        debug!("Removing drag hooks from {id:?}");
        world.commands().entity(id).on::<Self>().despawn_children();
    }
}

/// Marker component for entities with OOB alpha scaling applied,
/// so that we can track when scaling needs to be removed.
#[derive(Debug, Component)]
struct OobScaled;

/// System that scales the alpha of entities dragged out of bounds.
///
/// TODO: Replace with a better modifier system.
#[allow(clippy::type_complexity)]
fn apply_oob_alpha(
    mut commands: Commands,
    mut q: Query<(Entity, Option<&mut AlphaScale>), (With<OutOfBounds>, Without<OobScaled>)>,
) {
    for (entity, alpha) in &mut q {
        if let Some(mut alpha) = alpha {
            alpha.0 *= OOB_ALPHA_FACTOR;
        }
        commands.entity(entity).insert(OobScaled);
    }
}

/// System that un-scales the alpha of entities dragged back inbounds.
///
/// TODO: Replace with a better modifier system.
#[allow(clippy::type_complexity)]
fn remove_oob_alpha(
    mut commands: Commands,
    mut q: Query<(Entity, Option<&mut AlphaScale>), (With<OobScaled>, Without<OutOfBounds>)>,
) {
    for (entity, alpha) in &mut q {
        if let Some(mut alpha) = alpha {
            alpha.0 /= OOB_ALPHA_FACTOR;
        }
        commands.entity(entity).remove::<OobScaled>();
    }
}

/// Plugin for cursor features.
pub struct DragPlugin;

impl Plugin for DragPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, drag_update_oob)
            .add_systems(
                PostUpdate,
                apply_oob_alpha.run_if(any_with_component::<OutOfBounds>),
            )
            .add_systems(
                PostUpdate,
                remove_oob_alpha.run_if(any_component_removed::<OutOfBounds>),
            );
    }
}

pub fn plugin() -> DragPlugin {
    DragPlugin
}
