use crate::components::player::Player;
use bevy::prelude::*;

pub fn camera_follow(
    player_query: Query<&Transform, With<Player>>,
    mut camera_query: Query<&mut Transform, (With<Camera2d>, Without<Player>)>,
) {
    if let Ok(player_transform) = player_query.single() {
        for mut camera_transform in camera_query.iter_mut() {
            camera_transform.translation = player_transform.translation.with_z(100.0);
        }
    }
}
