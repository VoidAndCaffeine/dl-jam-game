use crate::components::hit_effects::HitSpark;
use crate::constants::{
    HIT_SPARK_CORE_COLOR, HIT_SPARK_CORE_RADIUS, HIT_SPARK_DURATION, HIT_SPARK_FRINGE_COLOR,
    HIT_SPARK_FRINGE_RADIUS, HIT_SPARK_Z,
};
use crate::events::HitConfirm;
use crate::resources::circle_art::CircleAccess;
use bevy::ecs::message::MessageReader;
use bevy::prelude::*;

/// Spawns an impact spark for every confirmed hit: a yellow circle with a red
/// core, both drawn as procedural circles.
pub fn spawn_hit_sparks(
    mut commands: Commands,
    mut hits: MessageReader<HitConfirm>,
    mut circles: CircleAccess,
) {
    for hit in hits.read() {
        let fringe = circles.circle(HIT_SPARK_FRINGE_COLOR);
        let core = circles.circle(HIT_SPARK_CORE_COLOR);

        let mut spark = commands.spawn((
            HitSpark {
                remaining: HIT_SPARK_DURATION,
                total: HIT_SPARK_DURATION,
            },
            Transform::from_translation(hit.position.extend(HIT_SPARK_Z))
                .with_scale(Vec3::splat(HIT_SPARK_FRINGE_RADIUS)),
            Name::new("Hit Spark"),
        ));
        if let Some((mesh, material)) = fringe {
            spark.insert((mesh, material));
        }

        // The core is a child of the (scaled) fringe, so its local scale has to
        // cancel the fringe's scale to land on the wanted world radius.
        let core_scale = HIT_SPARK_CORE_RADIUS / HIT_SPARK_FRINGE_RADIUS;
        spark.with_children(|parent| {
            let mut child = parent.spawn((
                Transform::from_xyz(0.0, 0.0, 0.1).with_scale(Vec3::splat(core_scale)),
                Name::new("Hit Spark Core"),
            ));
            if let Some((mesh, material)) = core {
                child.insert((mesh, material));
            }
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
    use crate::resources::circle_art::CircleArt;
    use bevy::transform::TransformPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<ColorMaterial>>()
            .init_resource::<CircleArt>()
            .add_plugins((MinimalPlugins, TransformPlugin))
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
    fn a_spark_draws_as_circles_when_a_renderer_is_present() {
        let mut app = setup_app();
        let target = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(HitConfirm {
            target,
            position: Vec2::ZERO,
        });
        app.update();

        let spark = app
            .world_mut()
            .query_filtered::<Entity, With<HitSpark>>()
            .iter(app.world())
            .next()
            .unwrap();
        assert!(
            app.world().get::<Mesh2d>(spark).is_some(),
            "the fringe is a circle mesh"
        );
        let core = app
            .world()
            .get::<Children>(spark)
            .unwrap()
            .iter()
            .next()
            .unwrap();
        assert!(
            app.world().get::<Mesh2d>(core).is_some(),
            "the core is a circle mesh"
        );
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
                Transform::default(),
            ))
            .id();
        assert!(app.world().get::<HitSpark>(expired).is_some());

        app.update();

        assert!(app.world().get::<HitSpark>(expired).is_none());
    }
}
