//! Utilities for working with cursor manipulation.

use bevy::prelude::*;
use bevy_mod_picking::prelude::*;

/// Callback to allow dragging the listener entity around.
///
/// This is intended to be used in conjunction with [[bevy_mod_picking::prelude::On::run]],
/// on the [[Drag]] event.
/// It converts the cursor delta into world coordinates and applies the resulting delta to
/// the [[Transform]] of the listener entity (not the target entity).
///
/// Will panic if there is more than one camera.
pub fn drag_listener(
    event: Listener<Pointer<Drag>>,
    mut transform_query: Query<&mut Transform>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
) {
    let mut transform = transform_query
        .get_mut(event.listener())
        .expect("drag event applied to entity without a Transform component");
    let (camera, camera_transform) = camera_query.single();
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
