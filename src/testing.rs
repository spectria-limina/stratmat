use avian2d::prelude::*;
use bevy::{
    ecs::query::QuerySingleError,
    input::{mouse::MouseButtonInput, ButtonState},
    prelude::*,
    window::{PrimaryWindow, WindowEvent},
};
use itertools::Itertools;

use crate::Layer;

/// Adds a new test camera to the world, configured such that world and viewport coordinate systems are identical.
pub fn add_test_camera(mut commands: Commands, win_q: Query<&Window, With<PrimaryWindow>>) {
    let win_rect = Rect::new(0.0, 0.0, win_q.single().width(), win_q.single().height());
    debug!(
        "spawning test camera: dimensions: {}, position: {}",
        win_rect.size(),
        win_rect.center(),
    );
    commands.spawn((Camera2d, OrthographicProjection::default_2d(), Transform {
        translation: win_rect.center().extend(0.0),
        scale: Vec3::new(1.0, -1.0, 1.0),
        rotation: Quat::IDENTITY,
    }));
}

#[derive(Bundle, Default)]
pub struct DragSurfaceBundle {
    sprite: Sprite,
    transform: Transform,
    pickable: PickingBehavior,
    collider: Collider,
    layers: CollisionLayers,
}

impl DragSurfaceBundle {
    pub fn new(rect: Rect) -> Self {
        Self {
            sprite: Sprite {
                custom_size: Some(rect.size()),
                ..default()
            },
            transform: Transform::from_translation(rect.center().extend(0.0)),
            pickable: PickingBehavior::IGNORE,
            collider: Collider::rectangle(rect.width(), rect.height()),
            layers: CollisionLayers::new(Layer::DragSurface, LayerMask::ALL),
        }
    }
}

/// Generates a series of input events to simulate a mouse drag in the primary window.
///
/// Only one of these can be active at a time.
#[derive(Component, Debug)]
pub struct MockDrag {
    /// Positions are in world coordinates
    pub start_pos: Vec2,
    /// Positions are in world coordinates
    pub end_pos: Vec2,
    pub button: MouseButton,
    /// Number of frames over which the motion should occur.
    pub duration: f32,
}

/// Stores the current state of a [MockDrag] operation.
#[derive(Component, Default, Debug)]
pub struct MockDragState {
    pub tick: f32,
    pub pos: Vec2,
}

impl MockDrag {
    /// Fire mouse events and update state. Despawns the [MockDrag] entity once it's complete.
    ///
    /// Panics if multiple [MockDrag]s exist, or if there is not exactly one primary window.
    pub fn update(
        mut q: Query<(Entity, &MockDrag, Option<&mut MockDragState>)>,
        win_q: Query<Entity, With<PrimaryWindow>>,
        mut commands: Commands,
        mut window_ev: EventWriter<WindowEvent>,
    ) {
        let win = win_q.single();
        match q.get_single_mut() {
            Ok((id, drag, None)) => {
                // On the first frame, we must hover over the start position without clicking to generate hits.
                // Thanks egui.
                commands.entity(id).insert(MockDragState {
                    pos: drag.start_pos,
                    ..default()
                });
                window_ev.send(WindowEvent::CursorMoved(CursorMoved {
                    window: win,
                    position: drag.start_pos,
                    delta: None,
                }));
                debug!("beginning mock drag at {}", drag.start_pos);
            }
            Ok((id, drag, Some(ref mut state))) => {
                if state.tick == 0.0 {
                    window_ev.send(WindowEvent::MouseButtonInput(MouseButtonInput {
                        window: win,
                        button: drag.button,
                        state: ButtonState::Pressed,
                    }));
                    state.tick += 1.0;
                    return;
                }
                if state.tick / drag.duration >= 1.0 + 0.0001 {
                    window_ev.send(WindowEvent::MouseButtonInput(MouseButtonInput {
                        window: win,
                        button: drag.button,
                        state: ButtonState::Released,
                    }));
                    debug!("ending mock drag");
                    commands.entity(id).despawn();
                    return;
                }
                let progress = f32::min(state.tick / drag.duration, 1.0);
                let pos = drag.start_pos.lerp(drag.end_pos, progress);
                window_ev.send(WindowEvent::CursorMoved(CursorMoved {
                    window: win,
                    position: pos,
                    delta: Some(pos - state.pos),
                }));
                state.tick += 1.0;
                state.pos = pos;
                debug!("continuing mock drag to {}", pos);
            }
            Err(QuerySingleError::MultipleEntities(s)) => {
                panic!("can only process one MockDrag at a time: {s}")
            }
            Err(QuerySingleError::NoEntities(_)) => {}
        }
    }
}

pub fn forward_window_events(
    mut reader: EventReader<WindowEvent>,
    mut cursor_moved: EventWriter<CursorMoved>,
    mut mouse_button_input: EventWriter<MouseButtonInput>,
) {
    for ev in reader.read() {
        match ev {
            WindowEvent::CursorMoved(ev) => {
                cursor_moved.send(ev.clone());
            }
            WindowEvent::MouseButtonInput(ev) => {
                mouse_button_input.send(*ev);
            }
            _ => {}
        }
    }
}

#[track_caller]
pub fn debug_entities(world: &World) {
    for e in world.iter_entities() {
        debug!(
            "{:?}: {:?}",
            e.id(),
            world.inspect_entity(e.id()).map(|c| c.name()).collect_vec()
        );
    }
}
