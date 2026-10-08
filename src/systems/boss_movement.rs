use crate::components::boss::{Boss, Dying, PendingBlink, SurgeCharger};
use crate::components::boss_animation::BossAnimation;
use crate::components::player::Player;
use crate::constants::*;
use crate::levels::grid::SolidGrid;
use crate::resources::boss_encounter::BossCoordinator;
use crate::states::Phase;
use bevy::prelude::*;

/// Which way a boss steps this frame: `1` toward the player, `-1` away, `0`
/// hold position.
///
/// A lone boss closes to its standoff range and holds. In a dual fight the
/// attacker closes in, while the other half only backs off when the player
/// crowds it — otherwise both hold. The wide dead zones are hysteresis: once a
/// boss is comfortable it stays put instead of twitching in and out of the Walk
/// clip as the player shuffles.
pub fn step_sign(distance: f32, acting: bool, dual: bool) -> f32 {
    if dual {
        if acting {
            if distance > BOSS_DUAL_APPROACH_DISTANCE + BOSS_DUAL_DEAD_ZONE {
                1.0
            } else {
                0.0
            }
        } else if distance < BOSS_DUAL_RETREAT_DISTANCE - BOSS_DUAL_DEAD_ZONE {
            -1.0
        } else {
            0.0
        }
    } else if distance > BOSS_STANDOFF_DISTANCE + BOSS_MOVE_DEAD_ZONE {
        1.0
    } else {
        0.0
    }
}

/// Walks bosses around the arena.
///
/// Bosses that are stunned, charging, blinking, channelling or planted mid-hit
/// stay rooted. See [`step_sign`] for the approach/retreat rules.
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

        let sign = step_sign(distance, acting, coordinator.active);
        if sign == 0.0 {
            continue;
        }

        let speed = BOSS_MOVE_SPEED * boss.speed_multiplier();
        let half = Vec2::splat(boss.id.size() * 0.5);
        let step = direction * sign * speed * dt;
        let moved = grid.move_and_collide(half, from, step);
        transform.translation.x = moved.x;
        transform.translation.y = moved.y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lone_boss_closes_in_then_holds() {
        assert_eq!(step_sign(500.0, false, false), 1.0);
        assert_eq!(step_sign(BOSS_STANDOFF_DISTANCE, false, false), 0.0);
        // Inside the hysteresis band it holds rather than twitching.
        assert_eq!(
            step_sign(
                BOSS_STANDOFF_DISTANCE + BOSS_MOVE_DEAD_ZONE - 1.0,
                false,
                false
            ),
            0.0
        );
    }

    #[test]
    fn the_dual_attacker_closes_and_the_other_backs_off() {
        // A far player pulls the attacker in and lets the other hold.
        assert_eq!(step_sign(BOSS_DUAL_RETREAT_DISTANCE, true, true), 1.0);
        assert_eq!(step_sign(BOSS_DUAL_RETREAT_DISTANCE, false, true), 0.0);
        // A crowding player pushes the non-attacker away and lets the attacker hold.
        assert_eq!(step_sign(BOSS_DUAL_APPROACH_DISTANCE, false, true), -1.0);
        assert_eq!(step_sign(BOSS_DUAL_APPROACH_DISTANCE, true, true), 0.0);
    }

    #[test]
    fn the_dual_half_holds_in_a_wide_band() {
        // The attacker does not inch forward until well outside its range.
        assert_eq!(
            step_sign(
                BOSS_DUAL_APPROACH_DISTANCE + BOSS_DUAL_DEAD_ZONE - 1.0,
                true,
                true
            ),
            0.0
        );
        // The other half does not flee until the player is well inside its range.
        assert_eq!(
            step_sign(
                BOSS_DUAL_RETREAT_DISTANCE - BOSS_DUAL_DEAD_ZONE + 1.0,
                false,
                true
            ),
            0.0
        );
    }

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
