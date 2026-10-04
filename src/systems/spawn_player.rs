use crate::components::collider::Collider;
use crate::components::player::{Health, Movement, Player};
use crate::resources::level::PlayerSpawn;
use bevy::prelude::*;

pub fn spawn_player(mut commands: Commands, spawn: Res<PlayerSpawn>) {
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
        Transform::from_xyz(spawn.position.x, spawn.position.y, 1.0),
        Name::new("Player"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels::LevelId;
    use crate::resources::level::ActiveLevel;
    use crate::resources::level::build_level;

    #[test]
    fn the_player_appears_on_the_level_spawn_marker() {
        let mut app = App::new();
        let (def, grid) = build_level(LevelId::Farm);
        let expected = crate::resources::level::prop_position(
            &grid,
            &def,
            crate::levels::PropKind::PlayerSpawn,
        )
        .unwrap();

        app.insert_resource(ActiveLevel {
            id: LevelId::Farm,
            def,
        });
        app.insert_resource(grid);
        app.insert_resource(PlayerSpawn { position: expected });
        app.add_systems(Update, spawn_player);
        app.update();

        let transform = app
            .world_mut()
            .query_filtered::<&Transform, With<Player>>()
            .single(app.world())
            .unwrap();
        assert_eq!(transform.translation.truncate(), expected);
        assert_ne!(expected, Vec2::ZERO, "the spawn should not be the origin");
    }

    #[test]
    fn despawning_removes_the_player_and_camera() {
        let mut app = App::new();
        app.insert_resource(PlayerSpawn::default());
        app.add_systems(Update, spawn_player);
        app.update();
        assert_eq!(player_and_camera_count(&mut app), 2);

        app.add_systems(Update, despawn_player.after(spawn_player));
        app.update();
        assert_eq!(player_and_camera_count(&mut app), 0);
    }

    fn player_and_camera_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, Or<(With<Player>, With<Camera2d>)>>()
            .iter(app.world())
            .count()
    }
}
