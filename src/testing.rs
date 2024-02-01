use bevy::{
    ecs::query::QuerySingleError,
    input::{mouse::MouseButtonInput, ButtonState},
    prelude::*,
    render::camera::ScalingMode,
    window::PrimaryWindow,
};

use bevy_mod_picking::picking_core::Pickable;
use itertools::Itertools;

/// Adds a new test camera to the world, configured such that world and viewport coordinate systems are identical.
pub fn add_test_camera(mut commands: Commands, win_q: Query<&Window, With<PrimaryWindow>>) {
    let win_rect = Rect::new(0.0, 0.0, win_q.single().width(), win_q.single().height());
    log::debug!(
        "spawning test camera: dimensions: {}, position: {}",
        win_rect.size(),
        win_rect.center(),
    );
    commands.spawn(Camera2dBundle {
        projection: OrthographicProjection {
            near: -1000.0,
            far: 1000.0,
            scaling_mode: ScalingMode::WindowSize(1.0),
            ..default()
        },
        transform: Transform {
            translation: win_rect.center().extend(0.0),
            scale: Vec3::new(1.0, -1.0, 1.0),
            rotation: Quat::IDENTITY,
        },
        ..default()
    });
}

#[derive(Bundle, Default)]
pub struct DragSurfaceBundle {
    sprite: SpriteBundle,
    drag_surface: crate::cursor::DragSurface,
    pickable: Pickable,
}

impl DragSurfaceBundle {
    pub fn new(rect: Rect) -> Self {
        Self {
            sprite: SpriteBundle {
                sprite: Sprite {
                    custom_size: Some(rect.size()),
                    ..default()
                },
                transform: Transform::from_translation(rect.center().extend(0.0)),
                ..default()
            },
            pickable: Pickable::IGNORE,
            ..default()
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
}

impl MockDrag {
    /// Fire mouse events and update state. Despawns the [MockDrag] entity once it's complete.
    ///
    /// Panics if multiple [MockDrag]s exist, or if there is not exactly one primary window.
    pub fn update(
        mut q: Query<(Entity, &MockDrag, Option<&mut MockDragState>)>,
        win_q: Query<Entity, With<PrimaryWindow>>,
        mut commands: Commands,
        mut cursor_ev: EventWriter<CursorMoved>,
        mut button_ev: EventWriter<MouseButtonInput>,
    ) {
        let win = win_q.single();
        match q.get_single_mut() {
            Ok((id, drag, None)) => {
                // On the first frame, we must hover over the start position without clicking to generate hits.
                // Thanks egui.
                commands.entity(id).insert(MockDragState::default());
                cursor_ev.send(CursorMoved {
                    window: win,
                    position: drag.start_pos,
                });
                log::debug!("beginning mock drag at {}", drag.start_pos);
            }
            Ok((id, drag, Some(ref mut state))) => {
                if state.tick == 0.0 {
                    button_ev.send(MouseButtonInput {
                        window: win,
                        button: drag.button,
                        state: ButtonState::Pressed,
                    });
                    state.tick += 1.0;
                    return;
                }
                if state.tick / drag.duration >= 1.0 + 0.0001 {
                    button_ev.send(MouseButtonInput {
                        window: win,
                        button: drag.button,
                        state: ButtonState::Released,
                    });
                    log::debug!("ending mock drag");
                    commands.entity(id).despawn();
                    return;
                }
                let progress = f32::min(state.tick / drag.duration, 1.0);
                let position = drag.start_pos.lerp(drag.end_pos, progress);
                cursor_ev.send(CursorMoved {
                    window: win,
                    position,
                });
                state.tick += 1.0;
                log::debug!("continuing mock drag to {}", position);
            }
            Err(QuerySingleError::MultipleEntities(s)) => {
                panic!("can only process one MockDrag at a time: {s}")
            }
            Err(QuerySingleError::NoEntities(_)) => {}
        }
    }
}

#[track_caller]
pub fn debug_entities(world: &World) {
    for e in world.iter_entities() {
        log::debug!(
            "{:?}: {:?}",
            e.id(),
            world
                .inspect_entity(e.id())
                .iter()
                .map(|c| c.name())
                .collect_vec()
        );
    }
}
