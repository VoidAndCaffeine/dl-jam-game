use crate::components::collider::Collider;
use crate::components::player::{Health, Movement, Player};
use bevy::prelude::*;

#[derive(Component)]
pub struct StaticReference;

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

    // Static reference square for visual movement verification
    commands.spawn((
        StaticReference,
        Collider {
            size: Vec2::splat(32.0),
            is_solid: true,
        },
        Sprite {
            color: Color::srgb(1.0, 0.3, 0.3),
            custom_size: Some(Vec2::splat(32.0)),
            ..default()
        },
        Transform::from_xyz(200.0, 0.0, 0.0),
    ));
}

pub fn despawn_player(
    mut commands: Commands,
    player_query: Query<Entity, With<Player>>,
    camera_query: Query<Entity, With<Camera2d>>,
    static_query: Query<Entity, With<StaticReference>>,
) {
    for entity in player_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in camera_query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in static_query.iter() {
        commands.entity(entity).despawn();
    }
}
