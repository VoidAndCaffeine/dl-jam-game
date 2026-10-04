use crate::components::player::Player;
use crate::levels::grid::SolidGrid;
use crate::resources::camera::CameraFollowConfig;
use bevy::prelude::*;

pub fn camera_follow(
    time: Res<Time>,
    config: Res<CameraFollowConfig>,
    player_query: Query<&Transform, With<Player>>,
    mut camera_query: Query<&mut Transform, (With<Camera2d>, Without<Player>)>,
    grid: Option<Res<SolidGrid>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    let mut target_pos = player_transform.translation.truncate();
    if let Some(grid) = grid
        && grid.width() > 0
    {
        target_pos = grid.clamp_to_room(target_pos, config.visible_size);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels::grid::{SolidGrid, grid_from_rows};
    use crate::utils::level_parse::parse;

    fn room_grid() -> SolidGrid {
        let rows: Vec<String> = (0..10)
            .map(|y| {
                (0..10)
                    .map(|x| {
                        if x == 0 || x == 9 || y == 0 || y == 9 {
                            '#'
                        } else {
                            ','
                        }
                    })
                    .collect()
            })
            .collect();
        let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
        grid_from_rows(&refs, 32.0)
    }

    fn setup() -> App {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<CameraFollowConfig>()
            .insert_resource(room_grid());
        app.world_mut()
            .spawn((Player, Transform::from_xyz(0.0, 0.0, 1.0)));
        app.world_mut()
            .spawn((Camera2d, Transform::from_xyz(0.0, 0.0, 100.0)));
        app.add_systems(Update, camera_follow);
        app
    }

    #[test]
    fn the_camera_follows_the_player() {
        let mut app = setup();
        app.world_mut()
            .resource_mut::<CameraFollowConfig>()
            .visible_size = Vec2::splat(64.0);
        {
            let mut query = app
                .world_mut()
                .query_filtered::<&mut Transform, With<Player>>();
            query.single_mut(app.world_mut()).unwrap().translation.x = 64.0;
        }

        for _ in 0..200 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(16));
            app.update();
        }

        let camera = app
            .world_mut()
            .query_filtered::<&Transform, With<Camera2d>>()
            .single(app.world())
            .unwrap()
            .translation
            .truncate();
        assert!(
            camera.x > 40.0,
            "should catch up with the player, got {camera:?}"
        );
        assert_eq!(camera.y, 0.0);
    }

    #[test]
    fn a_view_wider_than_the_room_pins_the_camera_to_the_middle() {
        let mut app = setup();
        {
            let mut query = app
                .world_mut()
                .query_filtered::<&mut Transform, With<Player>>();
            query.single_mut(app.world_mut()).unwrap().translation.x = 10_000.0;
        }

        for _ in 0..50 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(16));
            app.update();
        }

        let camera = app
            .world_mut()
            .query_filtered::<&Transform, With<Camera2d>>()
            .single(app.world())
            .unwrap()
            .translation
            .truncate();
        assert_eq!(camera, Vec2::ZERO, "the room is smaller than the view");
    }

    #[test]
    fn the_camera_stays_inside_the_room() {
        let mut app = setup();
        app.world_mut()
            .resource_mut::<CameraFollowConfig>()
            .visible_size = Vec2::splat(64.0);

        {
            let mut query = app
                .world_mut()
                .query_filtered::<&mut Transform, With<Player>>();
            query.single_mut(app.world_mut()).unwrap().translation.x = 10_000.0;
        }

        for _ in 0..200 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(std::time::Duration::from_millis(16));
            app.update();
        }

        let grid = app.world().resource::<SolidGrid>().clone();
        let camera = app
            .world_mut()
            .query_filtered::<&Transform, With<Camera2d>>()
            .single(app.world())
            .unwrap()
            .translation
            .truncate();
        let limit = grid.world_size().x * 0.5 - 32.0;
        assert!(camera.x <= limit, "camera at {camera:?} left the room");
    }

    #[test]
    fn nothing_happens_without_a_player() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .init_resource::<CameraFollowConfig>()
            .insert_resource(
                parse("[tiles]\n##\n##\n")
                    .map(|def| SolidGrid::from_level(&def))
                    .unwrap(),
            );
        app.world_mut()
            .spawn((Camera2d, Transform::from_xyz(5.0, 5.0, 100.0)));
        app.add_systems(Update, camera_follow);
        app.update();

        let camera = *app
            .world_mut()
            .query_filtered::<&Transform, With<Camera2d>>()
            .single(app.world())
            .unwrap();
        assert_eq!(camera.translation.truncate(), Vec2::new(5.0, 5.0));
    }
}
