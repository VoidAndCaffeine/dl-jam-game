use crate::components::player::Player;
use crate::resources::camera::CameraFollowConfig;
use bevy::prelude::*;

pub fn camera_follow(
    config: Res<CameraFollowConfig>,
    player_query: Query<&Transform, With<Player>>,
    mut camera_query: Query<&mut Transform, (With<Camera2d>, Without<Player>)>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    for mut camera_transform in camera_query.iter_mut() {
        let camera_center = camera_transform.translation.truncate();
        let player_pos = player_transform.translation.truncate();
        let offset = player_pos - camera_center;

        let outside_x = offset.x.abs() > config.deadzone_size.x;
        let outside_y = offset.y.abs() > config.deadzone_size.y;

        if outside_x || outside_y {
            let target = player_transform.translation.with_z(100.0);
            camera_transform.translation = camera_transform
                .translation
                .lerp(target, config.lerp_factor);
        }
    }
}
