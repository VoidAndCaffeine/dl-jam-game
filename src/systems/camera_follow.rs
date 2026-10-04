use crate::components::player::Player;
use crate::resources::camera::CameraFollowConfig;
use bevy::prelude::*;

pub fn camera_follow(
    time: Res<Time>,
    config: Res<CameraFollowConfig>,
    player_query: Query<&Transform, With<Player>>,
    mut camera_query: Query<&mut Transform, (With<Camera2d>, Without<Player>)>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    let target_pos = player_transform.translation.truncate();
    let half_life = config.half_life.max(0.001);
    let damping = 4.0 * std::f32::consts::LN_2 / half_life;

    for mut camera_transform in camera_query.iter_mut() {
        let current_pos = camera_transform.translation.truncate();
        let displacement = target_pos - current_pos;

        let dt = time.delta_secs();
        let spring_force = displacement * (damping * damping * 0.25);
        let damper_force =
            -damping * (camera_transform.translation.truncate() - current_pos) / dt.max(0.001);

        let acceleration = spring_force;
        let velocity = (acceleration - damper_force) * dt;

        camera_transform.translation.x += velocity.x * dt;
        camera_transform.translation.y += velocity.y * dt;
        camera_transform.translation.z = 100.0;
    }
}
