use crate::components::collider::Collider;
use crate::components::player::{Health, Movement, Player};
use crate::components::player_sprite::{PLAYER_SPRITE_SIZE, PlayerAnimation};
use crate::resources::camera::CAMERA_ZOOM;
use crate::resources::level::PlayerSpawn;
use crate::resources::run_data::PlayerGear;
use bevy::prelude::*;

pub fn spawn_player(mut commands: Commands, spawn: Res<PlayerSpawn>, gear: Res<PlayerGear>) {
    let stats = gear.stats();
    commands.spawn((
        Player,
        PlayerAnimation::default(),
        Health::new(stats.max_health, stats.armor_reduction),
        Movement::default(),
        Collider {
            size: Vec2::splat(32.0),
            is_solid: true,
        },
        Sprite {
            color: Color::srgb(0.2, 0.6, 1.0),
            custom_size: Some(Vec2::splat(PLAYER_SPRITE_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn.position.x, spawn.position.y, 1.0),
        Name::new("Player"),
    ));

    // A 2x zoom so the world reads larger; this only affects rendering, not any
    // collision or interaction sizes.
    commands.spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection {
            scale: CAMERA_ZOOM,
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 100.0),
    ));
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
        app.init_resource::<PlayerGear>();
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
        app.init_resource::<PlayerGear>();
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

    #[test]
    fn the_player_starts_with_health_from_equipped_armor() {
        let mut app = App::new();
        app.insert_resource(PlayerSpawn::default());
        let mut gear = PlayerGear::default();
        let master_armor = crate::components::gear::GearPiece::new(
            crate::components::gear::GearSet::Master,
            crate::components::gear::GearSlot::Armor,
        );
        gear.own(master_armor);
        gear.equip(&master_armor);
        app.insert_resource(gear);
        app.add_systems(Update, spawn_player);
        app.update();

        let health = app
            .world_mut()
            .query_filtered::<&Health, With<Player>>()
            .single(app.world())
            .unwrap();
        assert_eq!(
            health.max,
            crate::components::gear::GearSet::Master.max_health()
        );
        assert_eq!(health.current, health.max);
        assert_eq!(
            health.armor_reduction,
            crate::components::gear::GearSet::Master.armor_reduction()
        );
    }

    #[test]
    fn the_camera_spawns_with_the_zoom_projection() {
        let mut app = App::new();
        app.insert_resource(PlayerSpawn::default());
        app.init_resource::<PlayerGear>();
        app.add_systems(Update, spawn_player);
        app.update();

        let projection = app
            .world_mut()
            .query_filtered::<&Projection, With<Camera2d>>()
            .single(app.world())
            .unwrap();
        match projection {
            Projection::Orthographic(ortho) => {
                assert_eq!(ortho.scale, crate::resources::camera::CAMERA_ZOOM)
            }
            other => panic!("expected an orthographic camera, got {other:?}"),
        }
    }
}
