use crate::components::player::{Movement, Player};
use bevy::prelude::*;

pub fn movement_physics(
    time: Res<Time>,
    mut player_query: Query<(&Movement, &mut Transform), With<Player>>,
    mut camera_query: Query<&mut Transform, (With<Camera2d>, Without<Player>)>,
) {
    let mut player_transform = None;

    for (movement, mut transform) in player_query.iter_mut() {
        let velocity = movement.input_direction * movement.speed;
        transform.translation.x += velocity.x * time.delta_secs();
        transform.translation.y += velocity.y * time.delta_secs();
        player_transform = Some(transform.translation);
    }

    if let Some(pos) = player_transform {
        for mut camera_transform in camera_query.iter_mut() {
            camera_transform.translation = pos.with_z(100.0);
        }
    }
}
