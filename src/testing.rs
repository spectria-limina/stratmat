use bevy::{prelude::*, render::camera::ScalingMode};

pub fn add_test_camera(mut commands: Commands) {
    commands.spawn(Camera2dBundle {
        projection: OrthographicProjection {
            near: -1000.0,
            far: 1000.0,
            scaling_mode: ScalingMode::Fixed {
                width: 1000.0,
                height: 1000.0,
            },
            ..default()
        },
        ..default()
    });
}
