use crate::components::boss::{Boss, BossBrain, BossId, DualRole, Dying, PatternType};
use crate::constants::{
    BOSS_PATTERN_COOLDOWN, BOSS_PATTERN_COOLDOWN_P2, DUAL_COMBO_GAP, DUAL_COMBO_INTERVAL,
    PHASE_STUN_DURATION, PHASE_THRESHOLD,
};
use crate::events::{BossAttackStarted, BossPhaseChanged, PlaySfx, Sfx};
use crate::levels::grid::SolidGrid;
use crate::resources::boss_encounter::{BossCoordinator, SharedBossHealth};
use crate::resources::boss_rng::BossRng;
use crate::states::Phase;
use crate::systems::boss_patterns::pattern_duration;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

/// The boss brain: watches health for phase changes and, when not stunned or
/// busy, commits to a pattern and announces it. Dual encounters are driven by
/// [`BossCoordinator`] instead, so both halves act on one schedule.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn boss_ai(
    time: Res<Time<Fixed>>,
    grid: Res<SolidGrid>,
    mut bosses: Query<(
        Entity,
        &mut Boss,
        &mut BossBrain,
        Option<&DualRole>,
        &mut Transform,
        &mut Sprite,
        Option<&Dying>,
    )>,
    shared: Res<SharedBossHealth>,
    mut coordinator: ResMut<BossCoordinator>,
    mut rng: ResMut<BossRng>,
    mut changed: MessageWriter<BossPhaseChanged>,
    mut started: MessageWriter<BossAttackStarted>,
    mut sfx: MessageWriter<PlaySfx>,
    phase: Phase,
) {
    if !phase.is_boss_fight() {
        return;
    }
    let dt = time.delta_secs();

    // Phase transitions and the stun flash, for both single and dual bosses.
    for (_, mut boss, _, role, _, mut sprite, dying) in bosses.iter_mut() {
        if dying.is_some() {
            continue;
        }
        let fraction = if role.is_some() {
            shared.fraction()
        } else {
            boss.health_fraction()
        };
        if boss.phase == 1 && fraction <= PHASE_THRESHOLD {
            boss.phase = 2;
            boss.stun_remaining = PHASE_STUN_DURATION;
            changed.write(BossPhaseChanged {
                boss_id: role.map(|dual_role| dual_role.id()).unwrap_or(boss.id),
                new_phase: 2,
            });
            sfx.write(PlaySfx(phase_two_cue(role, boss.id)));
        }
        if boss.stun_remaining > 0.0 {
            boss.stun_remaining = (boss.stun_remaining - dt).max(0.0);
            sprite.color = Color::WHITE;
        } else {
            sprite.color = display_color(role, boss.id);
        }
    }

    if coordinator.active {
        if coordinator.channelling {
            coordinator.channel_remaining -= dt;
            if coordinator.channel_remaining <= 0.0 {
                coordinator.channelling = false;
                coordinator.schedule_amalgamation(shared.fraction() <= PHASE_THRESHOLD);
            }
            return;
        }

        coordinator.next_combo -= dt;
        coordinator.next_amalgamation -= dt;
        if coordinator.next_combo > 0.0 {
            return;
        }
        coordinator.next_combo = DUAL_COMBO_INTERVAL;

        let amalgamation = coordinator.next_amalgamation <= 0.0;
        let center = grid.center();
        let mut excavator: Option<(Entity, u8)> = None;
        let mut quicksilver: Option<(Entity, u8)> = None;
        for (entity, boss, _, role, mut transform, _, dying) in bosses.iter_mut() {
            if dying.is_some() {
                continue;
            }
            match role {
                Some(DualRole::Excavator) => {
                    if amalgamation {
                        transform.translation.x = center.x - 48.0;
                        transform.translation.y = center.y;
                    }
                    excavator = Some((entity, boss.phase));
                }
                Some(DualRole::Quicksilver) => {
                    if amalgamation {
                        transform.translation.x = center.x + 48.0;
                        transform.translation.y = center.y;
                    }
                    quicksilver = Some((entity, boss.phase));
                }
                None => {}
            }
        }

        if amalgamation {
            coordinator.next_amalgamation = f32::MAX;
            coordinator.channelling = true;
            coordinator.channel_remaining = crate::constants::AMALGAMATION_CHANNEL;
            if let Some((entity, half_phase)) = excavator {
                started.write(BossAttackStarted {
                    entity,
                    boss_id: BossId::Dual,
                    pattern: PatternType::Amalgamation,
                    phase: half_phase,
                });
            }
            return;
        }

        // The halves take turns: only the one whose turn it is commits, while
        // the other keeps its distance (see `boss_movement`).
        let attacker = match coordinator.turn {
            DualRole::Excavator => excavator,
            DualRole::Quicksilver => quicksilver,
        };
        if let Some((entity, half_phase)) = attacker {
            let kit = coordinator.turn.id().patterns();
            let pattern = kit[rng.index(kit.len())];
            started.write(BossAttackStarted {
                entity,
                boss_id: BossId::Dual,
                pattern,
                phase: half_phase,
            });
            // Let the attack fully resolve before the other half steps up.
            coordinator.next_combo = pattern_duration(pattern, half_phase) + DUAL_COMBO_GAP;
            coordinator.flip_turn();
        }
        return;
    }

    let mut pending: Vec<(Entity, BossId, PatternType, u8)> = Vec::new();
    for (entity, boss, mut brain, role, _, _, dying) in bosses.iter_mut() {
        if role.is_some() || dying.is_some() || boss.stun_remaining > 0.0 {
            continue;
        }
        brain.cooldown = (brain.cooldown - dt).max(0.0);
        if brain.cooldown > 0.0 {
            continue;
        }
        let pick = pick_pattern(boss.id.patterns(), &brain, &mut rng);
        brain.current = Some(pick);
        brain.remember(pick);
        brain.cooldown = pattern_duration(pick, boss.phase) + cooldown_for(boss.phase);
        pending.push((entity, boss.id, pick, boss.phase));
    }
    for (entity, boss_id, pattern, pattern_phase) in pending {
        started.write(BossAttackStarted {
            entity,
            boss_id,
            pattern,
            phase: pattern_phase,
        });
    }
}

/// Picks a pattern, preferring ones not used in the last two choices so runs
/// stay varied. Falls back to the whole kit if every pattern is recent.
fn pick_pattern(patterns: &[PatternType], brain: &BossBrain, rng: &mut BossRng) -> PatternType {
    let fresh: Vec<PatternType> = patterns
        .iter()
        .copied()
        .filter(|pattern| !brain.recently_used(*pattern))
        .collect();
    let pool = if fresh.is_empty() { patterns } else { &fresh };
    pool[rng.index(pool.len())]
}

fn cooldown_for(phase: u8) -> f32 {
    if phase >= 2 {
        BOSS_PATTERN_COOLDOWN_P2
    } else {
        BOSS_PATTERN_COOLDOWN
    }
}

fn phase_two_cue(role: Option<&DualRole>, id: BossId) -> Sfx {
    match role {
        Some(DualRole::Quicksilver) => Sfx::QuicksilverPhase2,
        Some(DualRole::Excavator) => Sfx::ExcavatorPhase2,
        None if id == BossId::BossB => Sfx::QuicksilverPhase2,
        None => Sfx::ExcavatorPhase2,
    }
}

fn display_color(role: Option<&DualRole>, id: BossId) -> Color {
    match role {
        Some(role) => role.id().color(),
        None => id.color(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::boss::{AttackKind, BossAttack};

    #[test]
    fn pattern_picks_avoid_recent_history_when_it_can() {
        let patterns = PatternType::for_boss(BossId::BossA);
        let mut brain = BossBrain::default();
        brain.remember(PatternType::TailingsSurge);
        brain.remember(PatternType::ExcavatorSlam);
        let mut rng = BossRng::seeded(1);

        for _ in 0..32 {
            let pick = pick_pattern(patterns, &brain, &mut rng);
            assert_eq!(
                pick,
                PatternType::DebrisRain,
                "the only fresh pattern should be chosen while it lasts"
            );
        }
    }

    #[test]
    fn boss_attack_builder_defaults_are_armed_and_sane() {
        let attack: BossAttack = BossAttack::new(AttackKind::Slam, Vec2::new(3.0, 4.0));
        assert!(attack.armed);
        assert_eq!(attack.position, Vec2::new(3.0, 4.0));
        assert_eq!(attack.phase, 1);
    }
}
