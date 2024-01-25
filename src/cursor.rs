//! Utilities for working with cursor manipulation.
//!
//! Callbacks in this module are intended to be used in conjunction with [[bevy_mod_picking::prelude::On::run]]
//! in conjunction with various events.

use bevy::{prelude::*, render::primitives::Aabb};
use bevy_mod_picking::prelude::*;

use crate::color::HasColor;

/// The factor to apply to a sprite's alpha channel when it is dragged out of bounds.
const OOB_ALPHA_FACTOR: f32 = 0.1;

/// Callback to allow dragging the listener entity around.
///
/// It converts the cursor delta into world coordinates and applies the resulting delta to
/// the [[Transform]] of the listener entity (not the target entity).
///
/// It also applies or removes the [[OutOfBounds]] marker according to whether the dragged
/// entity is within the bounding box of a [[DragSurface]] or not, computed using the dragged
/// entity's translation.
///
/// TODO: Allow the dragged entity to have better collision logic than only checking its
/// translation once we have 0.13's bounding volume support.
///
/// Will panic if there is not exactly one camera.
pub fn drag_listener(
    event: Listener<Pointer<Drag>>,
    commands: Commands,
    mut transform_query: Query<(Entity, &mut Transform), Without<DragSurface>>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    surface_query: Query<(&Aabb, &GlobalTransform), With<DragSurface>>,
) {
    let (entity, mut transform) = transform_query
        .get_mut(event.listener())
        .expect("drag event applied to entity without a Transform component");
    let (camera, camera_transform) = camera_query.single();
    drag_update_transform(&event, &mut transform, camera, camera_transform);
    drag_update_oob(commands, entity, &transform, surface_query);
}

/// Update the given transform based on a [[Drag]] event.
fn drag_update_transform(
    event: &ListenerInput<Pointer<Drag>>,
    transform: &mut Transform,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) {
    let new_pos_viewport = event.pointer_location.position;
    let old_pos_viewport = new_pos_viewport - event.delta;
    let new_pos_world = camera
        .viewport_to_world_2d(camera_transform, new_pos_viewport)
        .expect("unable to map cursor position to world coordinates");
    let old_pos_world = camera
        .viewport_to_world_2d(camera_transform, old_pos_viewport)
        .expect("unable to map cursor position to world coordinates");
    let delta_world = new_pos_world - old_pos_world;
    transform.translation += Vec3::from((delta_world, 0.0));
}

fn drag_update_oob(
    mut commands: Commands,
    entity: Entity,
    transform: &Transform,
    surface_query: Query<(&Aabb, &GlobalTransform), With<DragSurface>>,
) {
    let mut on_surface = false;
    for (surface_aabb, surface_transform) in &surface_query {
        let surface_translation = surface_transform.translation().xy();
        let surface_rect = Rect::from_corners(
            surface_aabb.min().xy() + surface_translation,
            surface_aabb.max().xy() + surface_translation,
        );
        if surface_rect.contains(transform.translation.xy()) {
            on_surface = true;
        }
    }

    let mut commands = commands.get_entity(entity).unwrap();
    if on_surface {
        commands.remove::<OutOfBounds>();
    } else {
        commands.insert(OutOfBounds);
    }
}

/// When the listener entity is dropped [[OutOfBounds]], despawn it and its children.
pub fn despawn_dropped_oob(
    event: Listener<Pointer<DragEnd>>,
    mut commands: Commands,
    oob_query: Query<Entity, With<OutOfBounds>>,
) {
    let entity = event.listener();
    if oob_query.contains(entity) {
        commands.add(DespawnRecursive { entity })
    }
}

/// Marker component for entities that can have entities placed on them via dragging.
#[derive(Debug, Component)]
pub struct DragSurface;

/// Marker component for out-of-bounds entities.
///
/// When added or removed from an entity, the entity and all its children will have their
/// alpha scaled to make them appear faint while out of bounds.
#[derive(Debug, Component)]
pub struct OutOfBounds;

#[derive(Debug, Component)]
/// Marker component for draggable entities.
///
/// Do not insert this directly; use a [[DraggableBundle]] instead.
pub struct Draggable;

/// Bundle for entities that can be dragged onto [[DragSurface]]s.
#[derive(Bundle)]
pub struct DraggableBundle {
    draggable: Draggable,
    drag: On<Pointer<Drag>>,
    drag_end: On<Pointer<DragEnd>>,
}

impl DraggableBundle {
    pub fn new() -> Self {
        Self {
            draggable: Draggable,
            drag: On::<Pointer<Drag>>::run(drag_listener),
            drag_end: On::<Pointer<DragEnd>>::run(despawn_dropped_oob),
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
        commands.get_entity(entity).unwrap().insert(OobScaled);
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
        commands.get_entity(entity).unwrap().remove::<OobScaled>();
    }
}

/// Plugin for cursor features.
pub struct CursorPlugin;

impl Plugin for CursorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, apply_oob_alpha);
        app.add_systems(PostUpdate, remove_oob_alpha);
    }
}

pub fn plugin() -> CursorPlugin {
    CursorPlugin
}
