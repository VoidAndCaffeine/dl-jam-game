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
