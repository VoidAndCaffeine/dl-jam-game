use crate::components::collider::Collider;
use crate::components::player::{Movement, Player};
use crate::levels::grid::SolidGrid;
use crate::resources::player_status::PlayerStatus;
use bevy::prelude::*;

/// Moves the player against the level's solid tiles and any solid entity in the
/// room, replacing the old all-pairs entity sweep.
///
/// One box per solid entity is collected per frame, which is cheap at this
/// scale: a room holds a handful of pots and one crafting station.
pub fn grid_movement(
    time: Res<Time<Fixed>>,
    grid: Res<SolidGrid>,
    status: Res<PlayerStatus>,
    mut player: Query<(&Movement, &Collider, &mut Transform), With<Player>>,
    obstacles: Query<(&GlobalTransform, &Collider), Without<Player>>,
) {
    if grid.width() == 0 {
        return;
    }

    let boxes: Vec<(Vec2, Vec2)> = obstacles
        .iter()
        .filter(|(_, collider)| collider.is_solid)
        .map(|(transform, collider)| (transform.translation().truncate(), collider.size * 0.5))
        .collect();

    let speed_multiplier = status.speed_multiplier();
    for (movement, collider, mut transform) in player.iter_mut() {
        let half = collider.size * 0.5;
        let from = transform.translation.truncate();
        let velocity =
            movement.input_direction * (movement.speed * speed_multiplier) + movement.velocity;
        let delta = velocity * time.delta_secs();
        if delta == Vec2::ZERO {
            continue;
        }

        let moved = grid.move_and_collide_with(&boxes, half, from, delta);
        transform.translation.x = moved.x;
        transform.translation.y = moved.y;
    }
}
