use crate::components::boss::{AttackKind, Boss, BossAttack, PatternType, SurgeCharger};
use crate::components::player::Player;
use crate::components::targetable::{Decoy, Targetable};
use crate::constants::*;
use crate::events::{BossAttackStarted, PlaySfx, Sfx};
use crate::levels::grid::SolidGrid;
use crate::resources::boss_rng::BossRng;
use crate::systems::boss_animation::constrained_aim;
use crate::systems::boss_attacks::{attack_color, spawn_attack};
use bevy::prelude::*;

/// How long a boss is busy with a pattern before it may choose the next one.
pub fn pattern_duration(pattern: PatternType, phase: u8) -> f32 {
    let enraged = phase >= 2;
    match pattern {
        PatternType::TailingsSurge => {
            SURGE_WINDUP + SURGE_DURATION * if enraged { SURGE_CHARGES_P2 } else { 1 } as f32 + 0.4
        }
        PatternType::ExcavatorSlam => SLAM_WINDUP + 0.7,
        PatternType::DebrisRain => DEBRIS_FALL_TIME + 0.8,
        PatternType::MirrorStep => BLINK_TELL + 0.6,
        PatternType::QuicksilverWave => WAVE_WINDUP + 0.7,
        PatternType::MadnessSpray => SPRAY_WINDUP + 0.6,
        PatternType::Amalgamation => AMALGAMATION_CHANNEL + 0.6,
    }
}

/// Turns a committed pattern event into the hazards and projectiles it throws.
#[allow(clippy::too_many_arguments)]
pub fn spawn_pattern_attacks(
    mut commands: Commands,
    mut events: MessageReader<BossAttackStarted>,
    mut rng: ResMut<BossRng>,
    grid: Res<SolidGrid>,
    mut boss_transforms: Query<&mut Transform, (With<Boss>, Without<Player>)>,
    player: Query<&Transform, With<Player>>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let player_pos = player.single().map(|t| t.translation.truncate()).ok();

    for event in events.read() {
        let Ok(mut boss_transform) = boss_transforms.get_mut(event.entity) else {
            continue;
        };
        let boss_pos = boss_transform.translation.truncate();
        let aim = player_pos
            .map(|player| (player - boss_pos).normalize_or(Vec2::X))
            .unwrap_or(Vec2::X);
        let target = player_pos.unwrap_or(boss_pos);
        let enraged = event.phase >= 2;

        match event.pattern {
            PatternType::TailingsSurge => {
                let charges = if enraged { SURGE_CHARGES_P2 } else { 1 };
                commands.entity(event.entity).insert(SurgeCharger {
                    direction: constrained_aim(event.pattern, aim),
                    speed: SURGE_SPEED * if enraged { 1.15 } else { 1.0 },
                    charges_left: charges,
                    windup: SURGE_WINDUP,
                    active: SURGE_DURATION,
                    trail_timer: 0.0,
                    curve: if enraged {
                        SURGE_CURVE_DEG.to_radians()
                    } else {
                        0.0
                    },
                });
                sfx.write(PlaySfx(Sfx::SurgeWindup));
            }
            PatternType::ExcavatorSlam => {
                let radius = if enraged { SLAM_RADIUS_P2 } else { SLAM_RADIUS };
                let slam = BossAttack::new(AttackKind::Slam, target)
                    .owned_by(event.entity)
                    .with_phase(event.phase)
                    .with_radius(radius)
                    .with_damage(SLAM_DAMAGE)
                    .with_lifetime(SLAM_WINDUP + 0.3);
                spawn_attack(
                    &mut commands,
                    BossAttack {
                        armed: false,
                        ..slam
                    },
                    attack_color(AttackKind::Slam),
                    Vec2::splat(radius * 2.0),
                    0.0,
                );
                sfx.write(PlaySfx(Sfx::SlamWindup));
            }
            PatternType::DebrisRain => {
                let count = if enraged {
                    DEBRIS_COUNT_P2
                } else {
                    DEBRIS_COUNT
                };
                for _ in 0..count {
                    let offset =
                        Vec2::new(rng.signed_unit(), rng.signed_unit()) * DEBRIS_SPREAD * 0.5;
                    let spot = free_spot(&grid, target + offset, target);
                    let debris = BossAttack::new(AttackKind::Debris, spot)
                        .owned_by(event.entity)
                        .with_phase(event.phase)
                        .with_radius(DEBRIS_IMPACT_RADIUS)
                        .with_damage(DEBRIS_DAMAGE)
                        .with_lifetime(DEBRIS_FALL_TIME);
                    spawn_attack(
                        &mut commands,
                        BossAttack {
                            target: spot,
                            armed: false,
                            ..debris
                        },
                        attack_color(AttackKind::Debris),
                        Vec2::splat(DEBRIS_SHADOW_RADIUS * 2.0),
                        0.0,
                    );
                }
                sfx.write(PlaySfx(Sfx::DebrisWarning));
            }
            PatternType::MirrorStep => {
                let flank = Vec2::new(-aim.y, aim.x);
                let destination = free_spot(&grid, target + flank * (BLINK_RANGE * 0.5), boss_pos);
                boss_transform.translation.x = destination.x;
                boss_transform.translation.y = destination.y;
                sfx.write(PlaySfx(Sfx::Blink));

                let decoys = if enraged { DECOY_COUNT_P2 } else { 1 };
                for index in 0..decoys {
                    let angle = (index as f32) * std::f32::consts::TAU / decoys as f32;
                    let spot = target + Vec2::new(angle.cos(), angle.sin()) * 56.0;
                    let decoy = BossAttack::new(AttackKind::Decoy, spot)
                        .owned_by(event.entity)
                        .with_phase(event.phase)
                        .with_radius(DECOY_SIZE * 0.5)
                        .with_lifetime(6.0);
                    let entity = spawn_attack(
                        &mut commands,
                        decoy,
                        attack_color(AttackKind::Decoy),
                        Vec2::splat(DECOY_SIZE),
                        0.0,
                    );
                    commands.entity(entity).insert((
                        Targetable,
                        Decoy::new(DECOY_SPLASH_RADIUS, DECOY_SPLASH_DAMAGE),
                    ));
                    sfx.write(PlaySfx(Sfx::DecoySpawn));
                }
            }
            PatternType::QuicksilverWave => {
                let speed = if enraged { WAVE_SPEED_P2 } else { WAVE_SPEED };
                let wave = BossAttack::new(AttackKind::Wave, boss_pos)
                    .owned_by(event.entity)
                    .with_phase(event.phase)
                    .with_radius(WAVE_WIDTH * 0.5)
                    .with_damage(WAVE_DAMAGE)
                    .with_lifetime(WAVE_LIFE);
                spawn_attack(
                    &mut commands,
                    BossAttack {
                        direction: aim,
                        speed,
                        ..wave
                    },
                    attack_color(AttackKind::Wave),
                    Vec2::new(WAVE_WIDTH, WAVE_LENGTH),
                    aim.y.atan2(aim.x),
                );
                if enraged {
                    let cross = Vec2::new(-aim.y, aim.x);
                    let second = BossAttack::new(AttackKind::Wave, boss_pos)
                        .owned_by(event.entity)
                        .with_phase(event.phase)
                        .with_radius(WAVE_WIDTH * 0.5)
                        .with_damage(WAVE_DAMAGE)
                        .with_lifetime(WAVE_LIFE);
                    spawn_attack(
                        &mut commands,
                        BossAttack {
                            direction: cross,
                            speed,
                            ..second
                        },
                        attack_color(AttackKind::Wave),
                        Vec2::new(WAVE_WIDTH, WAVE_LENGTH),
                        cross.y.atan2(cross.x),
                    );
                }
                sfx.write(PlaySfx(Sfx::WaveLaunch));
            }
            PatternType::MadnessSpray => {
                let droplets = if enraged {
                    SPRAY_DROPLETS_P2
                } else {
                    SPRAY_DROPLETS
                };
                let spread = if enraged { 360.0 } else { SPRAY_CONE_DEG };
                let base_angle = aim.y.atan2(aim.x);
                for index in 0..droplets {
                    let t = if droplets > 1 {
                        index as f32 / (droplets - 1) as f32
                    } else {
                        0.5
                    };
                    let angle = base_angle + (-spread * 0.5 + spread * t).to_radians();
                    let direction = Vec2::new(angle.cos(), angle.sin());
                    let homing = enraged && index % 2 == 1;
                    let kind = if homing {
                        AttackKind::Wisp
                    } else {
                        AttackKind::Spray
                    };
                    let mut droplet = BossAttack::new(kind, boss_pos)
                        .owned_by(event.entity)
                        .with_phase(event.phase)
                        .with_radius(if homing { 9.0 } else { 7.0 })
                        .with_damage(SPRAY_DAMAGE)
                        .with_lifetime(if homing { WISP_LIFE } else { SPRAY_LIFE });
                    droplet.direction = direction;
                    droplet.speed = if homing { WISP_SPEED } else { SPRAY_SPEED };
                    spawn_attack(
                        &mut commands,
                        droplet,
                        attack_color(kind),
                        Vec2::splat(16.0),
                        angle,
                    );
                }
                sfx.write(PlaySfx(Sfx::SprayWindup));
                sfx.write(PlaySfx(Sfx::SprayRelease));
            }
            PatternType::Amalgamation => {
                let center = grid.center();
                let amalgam = BossAttack::new(AttackKind::Amalgam, center)
                    .with_phase(event.phase)
                    .with_radius(AMALGAMATION_RADIUS)
                    .with_damage(AMALGAMATION_DAMAGE)
                    .with_lifetime(AMALGAMATION_CHANNEL);
                spawn_attack(
                    &mut commands,
                    BossAttack {
                        armed: false,
                        ..amalgam
                    },
                    attack_color(AttackKind::Amalgam),
                    Vec2::splat(AMALGAMATION_RADIUS * 2.0),
                    0.0,
                );
                sfx.write(PlaySfx(Sfx::AmalgamWarning));
                sfx.write(PlaySfx(Sfx::AmalgamChannel));
            }
        }
    }
}

/// A candidate spot that is not inside a wall, falling back to `safe` if it is.
fn free_spot(grid: &SolidGrid, candidate: Vec2, safe: Vec2) -> Vec2 {
    if grid.aabb_hits_solid(candidate, Vec2::splat(16.0)) {
        safe
    } else {
        candidate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longer_patterns_are_busier_than_shorter_ones() {
        let surge = pattern_duration(PatternType::TailingsSurge, 1);
        let blink = pattern_duration(PatternType::MirrorStep, 1);
        assert!(surge > blink);
        assert!(surge > 0.0 && blink > 0.0);
    }

    #[test]
    fn phase_two_surge_runs_longer_for_two_charges() {
        assert!(
            pattern_duration(PatternType::TailingsSurge, 2)
                > pattern_duration(PatternType::TailingsSurge, 1)
        );
    }
}
