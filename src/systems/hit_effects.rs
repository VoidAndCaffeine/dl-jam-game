use crate::components::hit_effects::HitSpark;
use crate::constants::{
    HIT_SPARK_CORE_SIZE, HIT_SPARK_DURATION, HIT_SPARK_FRINGE_SIZE, HIT_SPARK_Z,
};
use crate::events::HitConfirm;
use bevy::ecs::message::MessageReader;
use bevy::prelude::*;

/// Spawns an impact spark for every confirmed hit.
pub fn spawn_hit_sparks(mut commands: Commands, mut hits: MessageReader<HitConfirm>) {
    for hit in hits.read() {
        commands
            .spawn((
                HitSpark {
                    remaining: HIT_SPARK_DURATION,
                    total: HIT_SPARK_DURATION,
                },
                Sprite {
                    color: Color::srgba(1.0, 0.88, 0.2, 0.9),
                    custom_size: Some(Vec2::splat(HIT_SPARK_FRINGE_SIZE)),
                    ..default()
                },
                Transform::from_translation(hit.position.extend(HIT_SPARK_Z)),
                Name::new("Hit Spark"),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Sprite {
                        color: Color::srgba(1.0, 0.24, 0.2, 0.95),
                        custom_size: Some(Vec2::splat(HIT_SPARK_CORE_SIZE)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, 0.1),
                    Name::new("Hit Spark Core"),
                ));
            });
    }
}

/// Despawns sparks once their short lifetime runs out.
pub fn tick_hit_sparks(
    mut commands: Commands,
    time: Res<Time>,
    mut sparks: Query<(Entity, &mut HitSpark)>,
) {
    let dt = time.delta_secs();
    for (entity, mut spark) in sparks.iter_mut() {
        spark.remaining -= dt;
        if spark.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::transform::TransformPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin))
            .add_message::<HitConfirm>()
            .add_systems(Update, (spawn_hit_sparks, tick_hit_sparks).chain());
        app.update();
        app
    }

    fn spark_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<HitSpark>>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn a_confirmed_hit_spawns_one_spark_with_a_core() {
        let mut app = setup_app();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(HitConfirm {
            target,
            position: Vec2::new(10.0, 5.0),
        });
        app.update();

        assert_eq!(spark_count(&mut app), 1);

        let spark = app
            .world_mut()
            .query_filtered::<Entity, With<HitSpark>>()
            .iter(app.world())
            .next()
            .unwrap();
        let transform = app.world().get::<Transform>(spark).unwrap();
        assert_eq!(transform.translation.truncate(), Vec2::new(10.0, 5.0));

        let children = app.world().get::<Children>(spark).unwrap();
        assert_eq!(children.iter().count(), 1, "spark has a core child");
    }

    #[test]
    fn a_spark_despawns_after_its_lifetime() {
        let mut app = setup_app();
        let expired = app
            .world_mut()
            .spawn((
                HitSpark {
                    remaining: 0.0,
                    total: HIT_SPARK_DURATION,
                },
                Sprite::default(),
                Transform::default(),
            ))
            .id();
        assert!(app.world().get::<HitSpark>(expired).is_some());

        app.update();

        assert!(app.world().get::<HitSpark>(expired).is_none());
    }
}
