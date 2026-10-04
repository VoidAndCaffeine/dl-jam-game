use crate::components::collider::Collider;
use crate::components::player::{Health, Movement, Player};
use bevy::prelude::*;

pub fn spawn_player(mut commands: Commands) {
    commands.spawn((
        Player,
        Health {
            current: 100.0,
            max: 100.0,
        },
        Movement::default(),
        Collider {
            size: Vec2::splat(32.0),
            is_solid: true,
        },
        Sprite {
            color: Color::srgb(0.2, 0.6, 1.0),
            custom_size: Some(Vec2::splat(32.0)),
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 1.0),
    ));

    commands.spawn((Camera2d, Transform::from_xyz(0.0, 0.0, 100.0)));
}

pub fn despawn_player(
    mut commands: Commands,
    player_query: Query<Entity, With<Player>>,
    camera_query: Query<Entity, With<Camera2d>>,
) {
    for entity in player_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in camera_query.iter() {
        commands.entity(entity).despawn();
    }
}
