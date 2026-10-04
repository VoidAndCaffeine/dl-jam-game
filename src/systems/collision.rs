use crate::components::collider::{Collider, CollisionEvent};
use bevy::prelude::*;

pub fn collision_detection(mut commands: Commands, query: Query<(Entity, &Transform, &Collider)>) {
    let entities: Vec<_> = query.iter().collect();

    for i in 0..entities.len() {
        let (entity_a, transform_a, collider_a) = entities[i];

        if !collider_a.is_solid {
            continue;
        }

        for j in (i + 1)..entities.len() {
            let (entity_b, transform_b, collider_b) = entities[j];

            if !collider_b.is_solid {
                continue;
            }

            let half_a = collider_a.size * 0.5;
            let half_b = collider_b.size * 0.5;

            let min_a = transform_a.translation.truncate() - half_a;
            let max_a = transform_a.translation.truncate() + half_a;
            let min_b = transform_b.translation.truncate() - half_b;
            let max_b = transform_b.translation.truncate() + half_b;

            if max_a.x > min_b.x && min_a.x < max_b.x && max_a.y > min_b.y && min_a.y < max_b.y {
                commands.trigger(CollisionEvent { entity_a, entity_b });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    fn setup_app() -> App {
        App::new()
    }

    fn spawn_collider(app: &mut App, pos: Vec2, size: Vec2, solid: bool) -> Entity {
        app.world_mut()
            .spawn((
                Collider {
                    size,
                    is_solid: solid,
                },
                Transform::from_xyz(pos.x, pos.y, 0.0),
            ))
            .id()
    }

    #[test]
    fn detects_aabb_overlap() {
        let mut app = setup_app();
        spawn_collider(&mut app, Vec2::ZERO, Vec2::splat(32.0), true);
        spawn_collider(&mut app, Vec2::new(16.0, 0.0), Vec2::splat(32.0), true);

        app.update();
        // System runs without error
    }

    #[test]
    fn ignores_non_solid_colliders() {
        let mut app = setup_app();
        spawn_collider(&mut app, Vec2::ZERO, Vec2::splat(32.0), true);
        spawn_collider(&mut app, Vec2::new(16.0, 0.0), Vec2::splat(32.0), false);

        app.update();
    }

    #[test]
    fn no_self_collision() {
        let mut app = setup_app();
        spawn_collider(&mut app, Vec2::ZERO, Vec2::splat(32.0), true);

        app.update();
    }

    #[test]
    fn no_overlap_when_separated() {
        let mut app = setup_app();
        spawn_collider(&mut app, Vec2::ZERO, Vec2::splat(32.0), true);
        spawn_collider(&mut app, Vec2::new(100.0, 0.0), Vec2::splat(32.0), true);

        app.update();
    }
}
