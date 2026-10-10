use crate::components::boss::{
    AttackKind, Boss, BossAttack, PatternType, PendingBlink, SurgeCharger,
};
use crate::components::boss_animation::ClipWindows;
use crate::components::player::{Movement, Player};
use crate::components::targetable::{Decoy, Targetable};
use crate::constants::*;
use crate::events::{BossAttackStarted, PlayLoopSfx, PlaySfx, Sfx};
use crate::levels::grid::SolidGrid;
use crate::resources::boss_rng::BossRng;
use crate::systems::boss_animation::constrained_aim;
use crate::systems::boss_attacks::spawn_attack;
use bevy::prelude::*;

/// How long each phase of an attack lasts, in seconds.
///
/// The windup is identical in both phases so the tell never changes; phase 2
/// only trims the recovery and, for the surge, adds a second charge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PatternTimings {
    pub windup: f32,
    pub active: f32,
    pub recovery: f32,
}

impl PatternTimings {
    pub fn total(&self) -> f32 {
        self.windup + self.active + self.recovery
    }
}

/// How much faster recovery/cooldown frames play in `phase`.
///
/// Speeding the recovery tightens the gap between attacks while leaving the
/// windup (the tell) at its authored frames.
fn recovery_speed(phase: u8) -> f32 {
    if phase >= 2 {
        BOSS_P2_RECOVERY_SPEED
    } else {
        BOSS_P1_RECOVERY_SPEED
    }
}

/// The phase timings for `pattern` at `phase`.
pub fn pattern_timings(pattern: PatternType, phase: u8) -> PatternTimings {
    let enraged = phase >= 2;
    let speed = recovery_speed(phase);
    match pattern {
        PatternType::TailingsSurge => {
            let charges = if enraged { SURGE_CHARGES_P2 } else { 1 };
            let rewinds = charges.saturating_sub(1) as f32;
            PatternTimings {
                windup: SURGE_WINDUP,
                active: SURGE_DURATION * charges as f32 + SURGE_REWINDUP * rewinds,
                recovery: SURGE_RECOVERY / speed,
            }
        }
        PatternType::ExcavatorSlam => PatternTimings {
            windup: SLAM_WINDUP,
            active: SLAM_ACTIVE,
            recovery: SLAM_RECOVERY / speed,
        },
        PatternType::DebrisRain => PatternTimings {
            windup: DEBRIS_WINDUP,
            active: DEBRIS_ACTIVE,
            recovery: DEBRIS_RECOVERY / speed,
        },
        PatternType::MirrorStep => PatternTimings {
            windup: BLINK_WINDUP,
            active: BLINK_VANISH,
            recovery: BLINK_RECOVERY / speed,
        },
        PatternType::QuicksilverWave => PatternTimings {
            windup: WAVE_WINDUP,
            active: WAVE_ACTIVE,
            recovery: WAVE_RECOVERY / speed,
        },
        PatternType::MadnessSpray => PatternTimings {
            windup: SPRAY_WINDUP,
            active: SPRAY_ACTIVE,
            recovery: SPRAY_RECOVERY / speed,
        },
        PatternType::Amalgamation => PatternTimings {
            windup: AMALGAMATION_CHANNEL,
            active: 0.0,
            recovery: 0.0,
        },
    }
}

/// How long a boss is busy with a pattern before it may choose the next one.
pub fn pattern_duration(pattern: PatternType, phase: u8) -> f32 {
    pattern_timings(pattern, phase).total()
}

/// Which frames (0-based, inclusive) belong to each phase of a clip.
///
/// These mirror the art: frame 9 of the surge charge is its last windup frame,
/// frames 10-12 are the charge pose that loops, and so on.
pub fn clip_windows(pattern: PatternType) -> ClipWindows {
    match pattern {
        PatternType::TailingsSurge => ClipWindows {
            windup: (0, 8),
            active: (9, 11),
            recovery: (12, 24),
        },
        PatternType::ExcavatorSlam => ClipWindows {
            windup: (0, 6),
            active: (7, 11),
            recovery: (12, 24),
        },
        PatternType::DebrisRain => ClipWindows {
            windup: (0, 6),
            active: (7, 10),
            recovery: (11, 24),
        },
        PatternType::MirrorStep => ClipWindows {
            windup: (0, 7),
            active: (8, 13),
            recovery: (14, 24),
        },
        PatternType::QuicksilverWave => ClipWindows {
            windup: (0, 8),
            active: (9, 9),
            recovery: (10, 24),
        },
        PatternType::MadnessSpray => ClipWindows {
            windup: (0, 6),
            active: (7, 11),
            recovery: (12, 24),
        },
        PatternType::Amalgamation => ClipWindows {
            windup: (0, 24),
            active: (24, 24),
            recovery: (24, 24),
        },
    }
}

/// Turns a committed pattern event into the telegraphs and projectiles it
/// throws. Attacks carry their own windup, so nothing here deals damage yet.
#[allow(clippy::too_many_arguments)]
pub fn spawn_pattern_attacks(
    mut commands: Commands,
    mut events: MessageReader<BossAttackStarted>,
    mut rng: ResMut<BossRng>,
    grid: Res<SolidGrid>,
    player: Query<(&Transform, &Movement), With<Player>>,
    boss_transforms: Query<&Transform, (With<Boss>, Without<Player>)>,
    mut sfx: MessageWriter<PlaySfx>,
    mut loop_sfx: MessageWriter<PlayLoopSfx>,
) {
    let player = player.single().ok();
    let player_pos = player.map(|(transform, _)| transform.translation.truncate());
    let player_velocity = player
        .map(|(_, movement)| movement.velocity)
        .unwrap_or(Vec2::ZERO);

    for event in events.read() {
        let Ok(boss_transform) = boss_transforms.get(event.entity) else {
            continue;
        };
        let boss_pos = boss_transform.translation.truncate();
        let aim = player_pos
            .map(|player| (player - boss_pos).normalize_or(Vec2::X))
            .unwrap_or(Vec2::X);
        // Aim (and telegraph) where the player is heading, so windups still lead.
        let lead = player_pos
            .map(|player| player + player_velocity * BOSS_AIM_LEAD)
            .unwrap_or(boss_pos);
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
                // The caustic-trail sizzle is driven by the charge itself (see
                // `drive_surge_sizzle`), so it starts and stops with the charger.
                sfx.write(PlaySfx(Sfx::SurgeWindup));
            }
            PatternType::ExcavatorSlam => {
                let radius = (if enraged { SLAM_RADIUS_P2 } else { SLAM_RADIUS }) * HITBOX_SHRINK;
                let slam = BossAttack::new(AttackKind::Slam, lead)
                    .owned_by(event.entity)
                    .with_phase(event.phase)
                    .with_radius(radius)
                    .with_damage(SLAM_DAMAGE)
                    .with_lifetime(SLAM_WINDUP + SLAM_ACTIVE)
                    .with_windup(SLAM_WINDUP);
                spawn_attack(&mut commands, slam, 0.0);
                sfx.write(PlaySfx(Sfx::SlamWindup));
            }
            PatternType::DebrisRain => {
                let count = if enraged {
                    DEBRIS_COUNT_P2
                } else {
                    DEBRIS_COUNT
                };
                for index in 0..count {
                    let offset =
                        Vec2::new(rng.signed_unit(), rng.signed_unit()) * DEBRIS_SPREAD * 0.5;
                    let spot = free_spot(&grid, lead + offset, lead);
                    // Stagger the impacts across the active window so the rain
                    // lands over frames 8-11 rather than all at once.
                    let t = if count > 1 {
                        index as f32 / (count - 1) as f32
                    } else {
                        0.0
                    };
                    let fall = DEBRIS_WINDUP + DEBRIS_ACTIVE * t;
                    let debris = BossAttack::new(AttackKind::Debris, spot)
                        .owned_by(event.entity)
                        .with_phase(event.phase)
                        .with_radius(DEBRIS_IMPACT_RADIUS * HITBOX_SHRINK)
                        .with_damage(DEBRIS_DAMAGE)
                        .with_lifetime(fall)
                        .with_windup(fall);
                    spawn_attack(
                        &mut commands,
                        BossAttack {
                            target: spot,
                            ..debris
                        },
                        0.0,
                    );
                }
                sfx.write(PlaySfx(Sfx::DebrisWarning));
            }
            PatternType::MirrorStep => {
                let flank = Vec2::new(-aim.y, aim.x);
                let destination = free_spot(&grid, lead + flank * (BLINK_RANGE * 0.5), boss_pos);
                // The blink is held back until the windup frames finish.
                commands.entity(event.entity).insert(PendingBlink {
                    windup: BLINK_WINDUP,
                    destination,
                    decoys: if enraged { DECOY_COUNT_P2 } else { 1 },
                    phase: event.phase,
                });
            }
            PatternType::QuicksilverWave => {
                let speed = if enraged { WAVE_SPEED_P2 } else { WAVE_SPEED };
                let mut spawn_wave = |direction: Vec2| {
                    let wave = BossAttack::new(AttackKind::Wave, boss_pos)
                        .owned_by(event.entity)
                        .with_phase(event.phase)
                        .with_radius(WAVE_WIDTH * 0.5)
                        .with_damage(WAVE_DAMAGE)
                        .with_lifetime(WAVE_WINDUP + WAVE_LIFE)
                        .with_windup(WAVE_WINDUP);
                    spawn_attack(
                        &mut commands,
                        BossAttack {
                            direction,
                            speed,
                            ..wave
                        },
                        direction.y.atan2(direction.x),
                    );
                };
                spawn_wave(aim);
                if enraged {
                    spawn_wave(Vec2::new(-aim.y, aim.x));
                }
                loop_sfx.write(PlayLoopSfx {
                    sfx: Sfx::WaveLaunch,
                    duration: WAVE_WINDUP + WAVE_LIFE,
                });
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
                    let offset = (-spread * 0.5 + spread * t).to_radians();
                    let angle = base_angle + offset;
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
                        .with_radius((if homing { 9.0 } else { 7.0 }) * HITBOX_SHRINK)
                        .with_damage(SPRAY_DAMAGE)
                        .with_lifetime(if homing { WISP_LIFE } else { SPRAY_LIFE })
                        .with_windup(SPRAY_WINDUP)
                        .with_aim_offset(offset);
                    droplet.direction = direction;
                    droplet.speed = if homing { WISP_SPEED } else { SPRAY_SPEED };
                    spawn_attack(&mut commands, droplet, angle);
                }
                sfx.write(PlaySfx(Sfx::SprayWindup));
                loop_sfx.write(PlayLoopSfx {
                    sfx: Sfx::SprayRelease,
                    duration: if enraged { WISP_LIFE } else { SPRAY_LIFE },
                });
            }
            PatternType::Amalgamation => {
                let center = grid.center();
                let amalgam = BossAttack::new(AttackKind::Amalgam, center)
                    .with_phase(event.phase)
                    .with_radius(AMALGAMATION_RADIUS * HITBOX_SHRINK)
                    .with_damage(AMALGAMATION_DAMAGE)
                    .with_lifetime(AMALGAMATION_CHANNEL + 0.4)
                    .with_windup(AMALGAMATION_CHANNEL);
                spawn_attack(&mut commands, amalgam, 0.0);
                sfx.write(PlaySfx(Sfx::AmalgamWarning));
                // The halves blink together as the channel begins.
                sfx.write(PlaySfx(Sfx::Blink));
                loop_sfx.write(PlayLoopSfx {
                    sfx: Sfx::AmalgamChannel,
                    duration: AMALGAMATION_CHANNEL,
                });
            }
        }
    }
}

/// Waits out a [`PendingBlink`], then jumps the boss and drops its decoys.
pub fn resolve_pending_blinks(
    mut commands: Commands,
    time: Res<Time>,
    player: Query<&Transform, (With<Player>, Without<Boss>)>,
    mut bosses: Query<(Entity, &mut PendingBlink, &mut Transform), Without<Player>>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let dt = time.delta_secs();
    let player_pos = player
        .single()
        .map(|transform| transform.translation.truncate())
        .ok();

    for (entity, mut blink, mut transform) in bosses.iter_mut() {
        blink.windup -= dt;
        if blink.windup > 0.0 {
            continue;
        }

        transform.translation.x = blink.destination.x;
        transform.translation.y = blink.destination.y;
        sfx.write(PlaySfx(Sfx::Blink));

        let target = player_pos.unwrap_or(blink.destination);
        for index in 0..blink.decoys {
            let angle = (index as f32) * std::f32::consts::TAU / blink.decoys as f32;
            let spot = target + Vec2::new(angle.cos(), angle.sin()) * 56.0;
            let decoy = BossAttack::new(AttackKind::Decoy, spot)
                .owned_by(entity)
                .with_phase(blink.phase)
                .with_radius(DECOY_SIZE * 0.5)
                .with_lifetime(6.0);
            let decoy_entity = spawn_attack(&mut commands, decoy, 0.0);
            commands.entity(decoy_entity).insert((Targetable, Decoy));
            sfx.write(PlaySfx(Sfx::DecoySpawn));
        }

        commands.entity(entity).remove::<PendingBlink>();
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

    #[test]
    fn windups_do_not_change_between_phases() {
        for pattern in [
            PatternType::TailingsSurge,
            PatternType::ExcavatorSlam,
            PatternType::DebrisRain,
            PatternType::MirrorStep,
            PatternType::QuicksilverWave,
            PatternType::MadnessSpray,
        ] {
            assert_eq!(
                pattern_timings(pattern, 1).windup,
                pattern_timings(pattern, 2).windup,
                "{pattern:?} windup must not change"
            );
        }
    }

    #[test]
    fn phase_two_recovery_is_faster() {
        for pattern in [
            PatternType::ExcavatorSlam,
            PatternType::DebrisRain,
            PatternType::QuicksilverWave,
            PatternType::MadnessSpray,
        ] {
            assert!(
                pattern_timings(pattern, 2).recovery < pattern_timings(pattern, 1).recovery,
                "{pattern:?} recovery should shrink in phase 2"
            );
        }
    }

    #[test]
    fn every_pattern_has_ordered_clip_windows() {
        for pattern in [
            PatternType::TailingsSurge,
            PatternType::ExcavatorSlam,
            PatternType::DebrisRain,
            PatternType::MirrorStep,
            PatternType::QuicksilverWave,
            PatternType::MadnessSpray,
        ] {
            let windows = clip_windows(pattern);
            assert!(windows.windup.0 <= windows.windup.1, "{pattern:?} windup");
            assert!(windows.active.0 <= windows.active.1, "{pattern:?} active");
            assert!(
                windows.recovery.0 <= windows.recovery.1,
                "{pattern:?} recovery"
            );
            assert!(windows.windup.1 < windows.active.0, "{pattern:?} gap a");
            assert!(windows.active.1 < windows.recovery.0, "{pattern:?} gap b");
        }
    }
}
