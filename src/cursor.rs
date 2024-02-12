//! Utilities for working with cursor manipulation.

use std::fmt::Debug;

use bevy::prelude::*;
use bevy_commandify::entity_command;
use bevy_mod_picking::prelude::*;
use bevy_xpbd_2d::prelude::*;

use crate::color::HasColor;
use crate::Layer;

/// The factor to apply to a sprite's alpha channel when it is dragged out of bounds.
const OOB_ALPHA_FACTOR: f32 = 0.1;

/// Callback to add the update collision mask and add the [`Dragged`] component to newly-dragged entities.
pub fn on_drag_start(event: Listener<Pointer<DragStart>>, mut commands: Commands) {
    let id = event.listener();
    debug!("dragging {id:?}");
    if let Some(mut entity) = commands.get_entity(id) {
        entity.start_drag();
    } else {
        debug!("but it doesn't exist");
    }
}

/// Implementation of [`on_drag_start`] as a [`Command`](bevy::ecs::system::Command).
#[entity_command]
pub fn start_drag(id: Entity, world: &mut World) {
    debug!("starting drag on {id:?}");
    let Some(mut entity) = world.get_entity_mut(id) else {
        return;
    };
    if !entity.contains::<Draggable>() {
        debug!("but it isn't draggable");
        return;
    }
    if let Some(mut layers) = entity.get_mut::<CollisionLayers>() {
        *layers = layers
            .add_group(Layer::Dragged)
            .add_mask(Layer::DragSurface);
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
    event: Listener<Pointer<Drag>>,
    mut q: Query<&mut Transform>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
) {
    trace!("drag_listener");
    let Ok(mut transform) = q.get_mut(event.listener()) else {
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
                if layers.contains_group(Layer::DragSurface) {
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
    event: Listener<Pointer<DragEnd>>,
    mut q: Query<(&mut CollisionLayers, Has<OutOfBounds>)>,
    mut commands: Commands,
) {
    let id = event.listener();
    debug!("ending drag on {id:?}");
    let Ok((mut layers, oob)) = q.get_mut(id) else {
        debug!("but it doesn't exist");
        return;
    };
    if oob {
        debug!("{id:?} dropped out of bounds, despawning");
        commands.entity(id).despawn_recursive();
    } else {
        *layers = layers
            .remove_mask(Layer::DragSurface)
            .remove_group(Layer::Dragged);
        commands.entity(id).remove::<Dragged>();
    }
}

#[derive(Component, Copy, Clone, Default, Debug)]
/// Marker component for draggable entities.
///
/// Do not insert this directly; use a [DraggableBundle] instead.
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

/// Bundle for entities that can be dragged onto [`DragSurface`](Layer::DragSurface)s.
///
/// To work properly, requires the entity also have a [`Collider`].
#[derive(Bundle)]
pub struct DraggableBundle {
    draggable: Draggable,
    drag_start: On<Pointer<DragStart>>,
    drag: On<Pointer<Drag>>,
    drag_end: On<Pointer<DragEnd>>,
}

impl DraggableBundle {
    pub fn new() -> Self {
        Self {
            draggable: Draggable,
            drag_start: On::<Pointer<DragStart>>::run(on_drag_start),
            drag: On::<Pointer<Drag>>::run(on_drag),
            drag_end: On::<Pointer<DragEnd>>::run(on_drag_end),
        }
    }
}

impl Default for DraggableBundle {
    fn default() -> Self {
        Self::new()
    }
}

/// Marker component for entities with OOB alpha scaling applied,
/// so that we can track when scaling needs to be removed.
#[derive(Debug, Component)]
struct OobScaled;

/// System that scales the alpha of entities dragged out of bounds.
///
/// TODO: Replace with a better modifier system.
fn apply_oob_alpha(
    mut commands: Commands,
    q: Query<Entity, (With<OutOfBounds>, Without<OobScaled>)>,
    mut color_q: Query<&mut dyn HasColor>,
    child_q: Query<&Children>,
) {
    for entity in &q {
        let mut iter = color_q.iter_many_mut(
            child_q
                .iter_descendants(entity)
                .chain(std::iter::once(entity)),
        );
        while let Some(colors) = iter.fetch_next() {
            for mut color in colors {
                let color = color.color_mut();
                color.set_a(color.a() * OOB_ALPHA_FACTOR);
            }
        }
        commands.entity(entity).insert(OobScaled);
    }
}

/// System that un-scales the alpha of entities dragged back inbounds.
///
/// TODO: Replace with a better modifier system.
fn remove_oob_alpha(
    mut commands: Commands,
    q: Query<Entity, (With<OobScaled>, Without<OutOfBounds>)>,
    mut color_q: Query<&mut dyn HasColor>,
    child_q: Query<&Children>,
) {
    for entity in &q {
        let mut iter = color_q.iter_many_mut(
            child_q
                .iter_descendants(entity)
                .chain(std::iter::once(entity)),
        );
        while let Some(colors) = iter.fetch_next() {
            for mut color in colors {
                let color = color.color_mut();
                color.set_a(color.a() / OOB_ALPHA_FACTOR);
            }
        }
        commands.entity(entity).remove::<OobScaled>();
    }
}

/// Plugin for cursor features.
pub struct CursorPlugin;

impl Plugin for CursorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, drag_update_oob)
            .add_systems(PostUpdate, apply_oob_alpha.run_if(any_with_component::<OutOfBounds>()))
            .add_systems(PostUpdate, remove_oob_alpha.run_if(any_component_removed::<OutOfBounds>()))
            // Prevent crashes due to despawned entities
            .add_systems(
                PreUpdate,
                apply_deferred
                    .after(bevy_picking_core::PickSet::PostFocus)
                    .before(bevy_eventlistener_core::EventListenerSet),
            )
            // Sequence drag events with command execution in between.
            .add_systems(
                PreUpdate,
                apply_deferred
                    .after(bevy_eventlistener_core::event_dispatcher::EventDispatcher::<Pointer<DragStart>>::cleanup)
                    .before(bevy_eventlistener_core::event_dispatcher::EventDispatcher::<Pointer<Drag>>::build)
            )
            .add_systems(
                PreUpdate,
                apply_deferred
                    .after(bevy_eventlistener_core::event_dispatcher::EventDispatcher::<Pointer<Drag>>::cleanup)
                    .before(bevy_eventlistener_core::event_dispatcher::EventDispatcher::<Pointer<DragEnd>>::build)
            );
    }
}

pub fn plugin() -> CursorPlugin {
    CursorPlugin
}
