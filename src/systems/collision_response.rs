use crate::components::collider::{Collider, CollisionEvent};
use crate::components::player::Player;
use bevy::prelude::*;

pub fn collision_response(
    trigger: On<CollisionEvent>,
    mut player_query: Query<(&mut Transform, &Collider), With<Player>>,
    static_query: Query<(&Transform, &Collider), (With<Collider>, Without<Player>)>,
) {
    let event = trigger.event();
    let is_a_player = player_query.contains(event.entity_a);
    let is_b_player = player_query.contains(event.entity_b);

    if !is_a_player && !is_b_player {
        return;
    }

    let (player_entity, static_entity) = if is_a_player {
        (event.entity_a, event.entity_b)
    } else {
        (event.entity_b, event.entity_a)
    };

    let Ok((mut player_transform, player_collider)) = player_query.get_mut(player_entity) else {
        return;
    };
    let Ok((static_transform, static_collider)) = static_query.get(static_entity) else {
        return;
    };

    let player_half = player_collider.size * 0.5;
    let static_half = static_collider.size * 0.5;

    let player_min = player_transform.translation.truncate() - player_half;
    let player_max = player_transform.translation.truncate() + player_half;
    let static_min = static_transform.translation.truncate() - static_half;
    let static_max = static_transform.translation.truncate() + static_half;

    let overlap_x = (player_max.x - static_min.x).min(static_max.x - player_min.x);
    let overlap_y = (player_max.y - static_min.y).min(static_max.y - player_min.y);

    if overlap_x < overlap_y {
        let player_center = player_transform.translation.truncate();
        let static_center = static_transform.translation.truncate();
        let direction = if player_center.x < static_center.x {
            -1.0
        } else {
            1.0
        };
        player_transform.translation.x =
            static_center.x + direction * (static_half.x + player_half.x);
    } else {
        let player_center = player_transform.translation.truncate();
        let static_center = static_transform.translation.truncate();
        let direction = if player_center.y < static_center.y {
            -1.0
        } else {
            1.0
        };
        player_transform.translation.y =
            static_center.y + direction * (static_half.y + player_half.y);
    }
}
