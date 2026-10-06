use crate::components::player::{Health, Movement, Player};
use crate::constants::{IFRAME_DURATION, KNOCKBACK_DECAY_RATE, KNOCKBACK_FORCE, MIN_DAMAGE};
use crate::events::PlayerDied;
use crate::levels::grid::aabbs_overlap;
use crate::states::Phase;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

/// Flat armor reduction, never dropping a hit below [`MIN_DAMAGE`].
pub fn damage_after_armor(raw: f32, armor_reduction: f32) -> f32 {
    (raw - armor_reduction).max(MIN_DAMAGE)
}

/// Whether a `reach` by `width` swing from `origin` along `facing` overlaps an
/// axis-aligned box at `target_center` with `target_half` extents.
///
/// The target is rotated into the swing's frame, giving an exact
/// oriented-rectangle-vs-box test in one cheap pass. The placeholder swing
/// graphic is drawn with the same rectangle, so what you see is what you hit.
pub fn swing_hits(
    origin: Vec2,
    facing: Vec2,
    reach: f32,
    width: f32,
    target_center: Vec2,
    target_half: Vec2,
) -> bool {
    let facing = if facing == Vec2::ZERO {
        Vec2::X
    } else {
        facing.normalize()
    };
    let angle = -facing.y.atan2(facing.x);
    let (sin, cos) = angle.sin_cos();
    let offset = target_center - origin;
    let local = Vec2::new(
        offset.x * cos - offset.y * sin,
        offset.x * sin + offset.y * cos,
    );
    let swing_center = Vec2::new(reach * 0.5, 0.0);
    let swing_half = Vec2::new(reach * 0.5, width * 0.5);
    aabbs_overlap(local, target_half, swing_center, swing_half)
}

/// Applies a hit to the player: armor, invulnerability and knockback.
///
/// Returns `true` when the hit is lethal. Boss attacks will call this once
/// their patterns land; it is kept public and pure so it can be tested alone.
pub fn hurt_player(
    health: &mut Health,
    movement: &mut Movement,
    raw_damage: f32,
    source: Vec2,
    target: Vec2,
) -> bool {
    if health.is_invulnerable() {
        return false;
    }
    health.current -= damage_after_armor(raw_damage, health.armor_reduction);
    health.iframe_remaining = IFRAME_DURATION;
    movement.velocity = (target - source).normalize_or_zero() * KNOCKBACK_FORCE;
    health.current <= 0.0
}

/// Runs down invulnerability timers and lets knockback bleed off.
pub fn tick_combat_timers(
    time: Res<Time>,
    mut players: Query<(&mut Health, &mut Movement), With<Player>>,
) {
    let dt = time.delta_secs();
    let decay = (1.0 - KNOCKBACK_DECAY_RATE * dt).clamp(0.0, 1.0);
    for (mut health, mut movement) in players.iter_mut() {
        if health.iframe_remaining > 0.0 {
            health.iframe_remaining = (health.iframe_remaining - dt).max(0.0);
        }
        movement.velocity *= decay;
        if movement.velocity.length_squared() < 0.01 {
            movement.velocity = Vec2::ZERO;
        }
    }
}

/// Reports a defeat once the player's health runs out during a fight.
pub fn player_death_check(
    players: Query<&Health, With<Player>>,
    mut died: MessageWriter<PlayerDied>,
    phase: Phase,
) {
    if !phase.is_boss_fight() {
        return;
    }
    if players.iter().any(|health| health.current <= 0.0) {
        died.write(PlayerDied);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armor_subtracts_flat_and_never_goes_below_one() {
        assert_eq!(damage_after_armor(22.0, 0.0), 22.0);
        assert_eq!(damage_after_armor(22.0, 5.0), 17.0);
        assert_eq!(damage_after_armor(22.0, 10.0), 12.0);
        assert_eq!(damage_after_armor(22.0, 15.0), 7.0);
        assert_eq!(damage_after_armor(3.0, 15.0), MIN_DAMAGE);
    }

    #[test]
    fn a_swing_reaches_straight_ahead() {
        let origin = Vec2::ZERO;
        assert!(swing_hits(
            origin,
            Vec2::X,
            80.0,
            40.0,
            Vec2::new(60.0, 0.0),
            Vec2::splat(16.0)
        ));
        assert!(!swing_hits(
            origin,
            Vec2::X,
            80.0,
            40.0,
            Vec2::new(140.0, 0.0),
            Vec2::splat(16.0)
        ));
    }

    #[test]
    fn a_swing_misses_behind_and_to_the_side() {
        let origin = Vec2::ZERO;
        assert!(!swing_hits(
            origin,
            Vec2::X,
            80.0,
            40.0,
            Vec2::new(-40.0, 0.0),
            Vec2::splat(16.0)
        ));
        assert!(!swing_hits(
            origin,
            Vec2::X,
            80.0,
            40.0,
            Vec2::new(40.0, 60.0),
            Vec2::splat(16.0)
        ));
    }

    #[test]
    fn a_swing_rotates_with_facing() {
        let target = Vec2::new(0.0, 60.0);
        assert!(swing_hits(
            Vec2::ZERO,
            Vec2::Y,
            80.0,
            40.0,
            target,
            Vec2::splat(16.0)
        ));
        assert!(!swing_hits(
            Vec2::ZERO,
            Vec2::X,
            80.0,
            40.0,
            target,
            Vec2::splat(16.0)
        ));
    }

    #[test]
    fn a_heavy_swing_is_wider_than_a_light_one() {
        let off_axis = Vec2::new(40.0, 30.0);
        let target_half = Vec2::splat(8.0);
        assert!(!swing_hits(
            Vec2::ZERO,
            Vec2::X,
            70.0,
            20.0,
            off_axis,
            target_half
        ));
        assert!(swing_hits(
            Vec2::ZERO,
            Vec2::X,
            70.0,
            70.0,
            off_axis,
            target_half
        ));
    }

    #[test]
    fn hurt_player_applies_armor_and_starts_invulnerability() {
        let mut health = Health::new(100.0, 5.0);
        let mut movement = Movement::default();

        let lethal = hurt_player(
            &mut health,
            &mut movement,
            22.0,
            Vec2::ZERO,
            Vec2::new(10.0, 0.0),
        );

        assert!(!lethal);
        assert_eq!(health.current, 83.0);
        assert!(health.is_invulnerable());
        assert_eq!(movement.velocity, Vec2::new(KNOCKBACK_FORCE, 0.0));
    }

    #[test]
    fn invulnerability_ignores_further_hits() {
        let mut health = Health::new(100.0, 0.0);
        health.iframe_remaining = 0.4;
        let mut movement = Movement::default();

        hurt_player(&mut health, &mut movement, 50.0, Vec2::ZERO, Vec2::X);

        assert_eq!(health.current, 100.0);
        assert_eq!(movement.velocity, Vec2::ZERO);
    }

    #[test]
    fn hurt_player_reports_a_lethal_hit() {
        let mut health = Health::new(10.0, 0.0);
        let mut movement = Movement::default();
        assert!(hurt_player(
            &mut health,
            &mut movement,
            50.0,
            Vec2::ZERO,
            Vec2::X
        ));
        assert!(health.current <= 0.0);
    }

    #[test]
    fn knockback_points_away_from_the_source() {
        let mut health = Health::new(100.0, 0.0);
        let mut movement = Movement::default();
        hurt_player(
            &mut health,
            &mut movement,
            10.0,
            Vec2::new(10.0, 0.0),
            Vec2::ZERO,
        );
        assert_eq!(movement.velocity, Vec2::new(-KNOCKBACK_FORCE, 0.0));
    }
}
