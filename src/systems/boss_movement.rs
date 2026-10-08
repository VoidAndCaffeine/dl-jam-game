use crate::components::boss::{Boss, Dying, PendingBlink, SurgeCharger};
use crate::components::boss_animation::BossAnimation;
use crate::components::player::Player;
use crate::constants::*;
use crate::levels::grid::SolidGrid;
use crate::resources::boss_encounter::BossCoordinator;
use crate::states::Phase;
use bevy::prelude::*;

/// Walks bosses around the arena.
///
/// A lone boss closes to its standoff range and holds so the player can still
/// reach it. During a dual encounter the halves stay in motion: the one that is
/// attacking lunges in, and the other backs out to the far side. Bosses that
/// are stunned, charging, blinking, channelling or planted mid-hit stay rooted.
#[allow(clippy::type_complexity)]
pub fn boss_movement(
    time: Res<Time<Fixed>>,
    phase: Phase,
    grid: Res<SolidGrid>,
    coordinator: Res<BossCoordinator>,
    player: Query<&Transform, (With<Player>, Without<Boss>)>,
    mut bosses: Query<
        (
            &Boss,
            &mut Transform,
            Option<&SurgeCharger>,
            Option<&Dying>,
            Option<&PendingBlink>,
            Option<&BossAnimation>,
        ),
        Without<Player>,
    >,
) {
    if !phase.is_boss_fight() {
        return;
    }
    let Ok(player_transform) = player.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();
    let dt = time.delta_secs();

    for (boss, mut transform, charging, dying, blinking, animation) in bosses.iter_mut() {
        if dying.is_some()
            || charging.is_some()
            || blinking.is_some()
            || boss.stun_remaining > 0.0
            || (coordinator.active && coordinator.channelling)
        {
            continue;
        }

        let acting = animation.is_some_and(BossAnimation::is_acting);
        let winding_up = animation.is_some_and(BossAnimation::is_winding_up);
        // Plant while the hit lands so a locked telegraph keeps its footing.
        if acting && !winding_up {
            continue;
        }

        let from = transform.translation.truncate();
        let to_player = player_pos - from;
        let distance = to_player.length();
        let direction = to_player.normalize_or(Vec2::ZERO);
        if direction == Vec2::ZERO {
            continue;
        }

        // The attacker closes in; in a dual fight the other half backs out.
        let (standoff, retreats) = if coordinator.active {
            if acting {
                (BOSS_DUAL_APPROACH_DISTANCE, false)
            } else {
                (BOSS_DUAL_RETREAT_DISTANCE, true)
            }
        } else {
            (BOSS_STANDOFF_DISTANCE, false)
        };

        let dead_zone = 6.0;
        let step_direction = if distance > standoff + dead_zone {
            direction
        } else if retreats && distance < standoff - dead_zone {
            -direction
        } else {
            Vec2::ZERO
        };
        if step_direction == Vec2::ZERO {
            continue;
        }

        let speed = BOSS_MOVE_SPEED * boss.speed_multiplier();
        let half = Vec2::splat(boss.id.size() * 0.5);
        let moved = grid.move_and_collide(half, from, step_direction * speed * dt);
        transform.translation.x = moved.x;
        transform.translation.y = moved.y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_boss_closes_the_gap_towards_the_player() {
        let grid = crate::levels::grid::grid_from_rows(
            &[
                "##########",
                "#........#",
                "#........#",
                "#........#",
                "#........#",
                "#........#",
                "#........#",
                "#........#",
                "#........#",
                "##########",
            ],
            32.0,
        );
        let boss_pos = grid.center();
        let player_pos = boss_pos + Vec2::new(200.0, 0.0);
        let half = Vec2::splat(BOSS_A_SIZE * 0.5);
        let moved = grid.move_and_collide(
            half,
            boss_pos,
            (player_pos - boss_pos).normalize() * BOSS_MOVE_SPEED * 0.1,
        );
        assert!(moved.x > boss_pos.x);
    }
}
