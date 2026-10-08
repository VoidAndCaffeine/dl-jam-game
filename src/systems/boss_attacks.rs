use crate::components::boss::{AttackKind, Boss, BossAttack, BossEncounterEntity, SurgeCharger};
use crate::components::effect_sprite::EffectSprite;
use crate::components::player::{Health, Movement, PLAYER_SIZE, Player};
use crate::constants::*;
use crate::events::{DamageDealt, PlaySfx, PlayerDied, Sfx};
use crate::levels::grid::SolidGrid;
use crate::resources::player_status::PlayerStatus;
use crate::systems::combat::{hurt_player, swing_hits};
use crate::utils::targeting::circles_overlap;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use std::collections::HashMap;

/// Spawns one attack entity carrying its effect sprite. Shared by the pattern
/// spawner and the surge charger so attacks always carry the same plumbing.
///
/// The sprite size and color are driven from the effect; [`super::effect_sprite`]
/// fills in the sheet once the asset server is available.
pub fn spawn_attack(commands: &mut Commands, attack: BossAttack, angle: f32) -> Entity {
    let position = attack.position;
    let effect = EffectSprite::new(attack.kind.windup_effect(), attack.phase);
    let size = effect.size;
    let z = effect.kind.z();
    commands
        .spawn((
            attack,
            effect,
            BossEncounterEntity,
            Sprite {
                color: Color::WHITE,
                custom_size: Some(size),
                ..default()
            },
            Transform::from_xyz(position.x, position.y, z)
                .with_rotation(Quat::from_rotation_z(angle)),
            Name::new("Boss Attack"),
        ))
        .id()
}

/// Applies a hit to the player if it lands, reporting the sound and any death.
fn damage_player(
    health: &mut Health,
    movement: &mut Movement,
    source: Vec2,
    target: Vec2,
    damage: f32,
    sfx: &mut MessageWriter<PlaySfx>,
    died: &mut MessageWriter<PlayerDied>,
) {
    if health.is_invulnerable() {
        return;
    }
    let lethal = hurt_player(health, movement, damage, source, target);
    sfx.write(PlaySfx(Sfx::PlayerHit));
    if lethal {
        died.write(PlayerDied);
    }
}

/// Rotates a unit direction by `angle` radians.
fn rotate(direction: Vec2, angle: f32) -> Vec2 {
    if angle == 0.0 {
        return direction;
    }
    (Quat::from_rotation_z(angle) * direction.extend(0.0)).truncate()
}

/// Whether a travelling wave overlaps the player.
///
/// The wave sprite is a `WAVE_WIDTH`-deep by `WAVE_LENGTH`-wide wall whose
/// length runs across the travel, so the hit test borrows exactly those extents
/// and stays centred on what is drawn.
fn wave_hits(position: Vec2, direction: Vec2, player_pos: Vec2, player_half: Vec2) -> bool {
    let release = position - direction * (WAVE_WIDTH * 0.5);
    swing_hits(
        release,
        direction,
        WAVE_WIDTH,
        WAVE_LENGTH,
        player_pos,
        player_half,
    )
}

/// Returns `true` if the attack would hit its owner boss at the given position
/// with the given radius. Uses a simple circle-circle check against the boss's
/// hitbox (boss size * 0.5).
fn attack_hits_owner(
    attack: &BossAttack,
    attack_pos: Vec2,
    attack_radius: f32,
    boss_map: &HashMap<Entity, (Vec2, f32)>,
) -> bool {
    if let Some(owner) = attack.owner
        && let Some((owner_pos, owner_radius)) = boss_map.get(&owner)
    {
        return circles_overlap(attack_pos, attack_radius, *owner_pos, *owner_radius);
    }
    false
}

/// Moves, ages and resolves every spawned boss attack.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn tick_boss_attacks(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<SolidGrid>,
    mut status: ResMut<PlayerStatus>,
    mut attacks: Query<(Entity, &mut BossAttack, &mut Transform), (Without<Player>, Without<Boss>)>,
    boss_transforms: Query<(Entity, &Transform), (With<Boss>, Without<Player>)>,
    mut player: Query<(&Transform, &mut Health, &mut Movement), With<Player>>,
    mut damage_events: MessageWriter<DamageDealt>,
    mut sfx: MessageWriter<PlaySfx>,
    mut died: MessageWriter<PlayerDied>,
) {
    let dt = time.delta_secs();
    let Ok((player_transform, mut health, mut movement)) = player.single_mut() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();
    let player_velocity = movement.velocity;
    let player_half = Vec2::splat(PLAYER_SIZE * 0.5);

    // Cache boss transforms for owner collision checks
    let boss_map: HashMap<Entity, (Vec2, f32)> = boss_transforms
        .iter()
        .map(|(entity, t)| {
            let pos = t.translation.truncate();
            // Use the boss's ID size for the hitbox radius
            let size = 64.0; // BOSS_SIZE constant
            (entity, (pos, size * 0.5))
        })
        .collect();

    for (entity, mut attack, mut transform) in attacks.iter_mut() {
        attack.hit_cooldown = (attack.hit_cooldown - dt).max(0.0);
        attack.aux_timer = (attack.aux_timer - dt).max(0.0);
        attack.remaining -= dt;

        // A winding-up attack only telegraphs: no motion and no damage yet.
        if !attack.armed {
            attack.windup = (attack.windup - dt).max(0.0);
            if attack.windup > 0.0 {
                continue;
            }
            attack.armed = true;
            // Thrown attacks leave from the boss's current spot and snap onto
            // the player as they release.
            if attack.kind.aims_at_player() {
                if let Some(owner) = attack.owner
                    && let Ok((_, owner_transform)) = boss_transforms.get(owner)
                {
                    attack.position = owner_transform.translation.truncate();
                    transform.translation.x = attack.position.x;
                    transform.translation.y = attack.position.y;
                }
                let predicted = player_pos + player_velocity * BOSS_AIM_LEAD;
                let base = (predicted - attack.position).normalize_or(attack.direction);
                attack.direction = rotate(base, attack.aim_offset);
                transform.rotation =
                    Quat::from_rotation_z(attack.direction.y.atan2(attack.direction.x));
            }
        }

        match attack.kind {
            AttackKind::SurgeTrail | AttackKind::AcidPool | AttackKind::MercuryPool => {
                let radius = attack.radius;
                let hits_player =
                    circles_overlap(attack.position, radius, player_pos, player_half.x);
                let hits_owner = attack_hits_owner(&attack, attack.position, radius, &boss_map);

                if hits_player && !hits_owner {
                    if attack.kind == AttackKind::MercuryPool {
                        status.apply_slip(SLOW_DURATION);
                    } else {
                        status.apply_slow(SLOW_DURATION);
                    }
                    if attack.hit_cooldown <= 0.0 {
                        attack.hit_cooldown = HAZARD_TICK;
                        damage_player(
                            &mut health,
                            &mut movement,
                            attack.position,
                            player_pos,
                            attack.damage,
                            &mut sfx,
                            &mut died,
                        );
                    }
                }
                if attack.remaining <= 0.0 {
                    commands.entity(entity).despawn();
                }
            }
            AttackKind::Slam => {
                if !attack.impacted {
                    attack.impacted = true;
                    sfx.write(PlaySfx(Sfx::SlamImpact));
                    if attack.phase >= 2 {
                        spawn_slam_aftermath(&mut commands, &attack);
                    }
                }
                // The quake stays live across its active frames.
                let hits_player =
                    circles_overlap(attack.position, attack.radius, player_pos, player_half.x);
                let hits_owner =
                    attack_hits_owner(&attack, attack.position, attack.radius, &boss_map);

                if attack.hit_cooldown <= 0.0 && hits_player && !hits_owner {
                    attack.hit_cooldown = HAZARD_TICK;
                    damage_player(
                        &mut health,
                        &mut movement,
                        attack.position,
                        player_pos,
                        attack.damage,
                        &mut sfx,
                        &mut died,
                    );
                }
                if attack.remaining <= 0.0 {
                    commands.entity(entity).despawn();
                }
            }
            AttackKind::Debris => {
                if !attack.impacted {
                    attack.impacted = true;
                    let hits_player =
                        circles_overlap(attack.target, attack.radius, player_pos, player_half.x);
                    let hits_owner =
                        attack_hits_owner(&attack, attack.target, attack.radius, &boss_map);

                    if hits_player && !hits_owner {
                        damage_player(
                            &mut health,
                            &mut movement,
                            attack.target,
                            player_pos,
                            attack.damage,
                            &mut sfx,
                            &mut died,
                        );
                    }
                    if let Some(owner) = attack.owner {
                        damage_events.write(DamageDealt {
                            target: owner,
                            amount: attack.damage,
                            raw: attack.damage,
                        });
                    }
                    sfx.write(PlaySfx(Sfx::DebrisImpact));
                    if attack.phase >= 2 {
                        let mist = BossAttack::new(AttackKind::SurgeTrail, attack.target)
                            .with_radius(SURGE_TRAIL_WIDTH_P2 * 0.5 * HITBOX_SHRINK)
                            .with_damage(attack.damage * 0.3)
                            .with_lifetime(SURGE_TRAIL_LIFE)
                            .with_phase(attack.phase);
                        spawn_attack(&mut commands, mist, 0.0);
                    }
                    // The landing hit is done; keep the entity alive just long
                    // enough for its dust cloud to play out.
                    attack.remaining = DEBRIS_DUST_LIFE;
                    attack.total = DEBRIS_DUST_LIFE;
                }
                if attack.remaining <= 0.0 {
                    commands.entity(entity).despawn();
                }
            }
            AttackKind::Decoy => {
                if attack.remaining <= 0.0 {
                    commands.entity(entity).despawn();
                }
            }
            AttackKind::Wave => {
                let step = attack.direction * attack.speed * dt;
                let moved = grid.move_and_collide(Vec2::splat(1.0), attack.position, step);
                attack.position = moved;
                transform.translation.x = moved.x;
                transform.translation.y = moved.y;

                let hits_player =
                    wave_hits(attack.position, attack.direction, player_pos, player_half);
                // For wave, check owner collision with a circle at the wave's position
                // using the wave's width as radius (conservative check)
                let hits_owner =
                    attack_hits_owner(&attack, attack.position, WAVE_WIDTH * 0.5, &boss_map);

                if attack.hit_cooldown <= 0.0 && hits_player && !hits_owner {
                    attack.hit_cooldown = HAZARD_TICK;
                    damage_player(
                        &mut health,
                        &mut movement,
                        attack.position,
                        player_pos,
                        attack.damage,
                        &mut sfx,
                        &mut died,
                    );
                }
                if attack.phase >= 2 && attack.aux_timer <= 0.0 {
                    attack.aux_timer = 0.9;
                    let pool = BossAttack::new(AttackKind::MercuryPool, attack.position)
                        .with_radius(MERCURY_POOL_RADIUS * HITBOX_SHRINK)
                        .with_damage(MERCURY_POOL_DAMAGE)
                        .with_lifetime(MERCURY_POOL_LIFE)
                        .with_phase(attack.phase);
                    spawn_attack(&mut commands, pool, 0.0);
                }
                if attack.remaining <= 0.0 {
                    commands.entity(entity).despawn();
                }
            }
            AttackKind::Spray | AttackKind::Wisp => {
                if attack.kind == AttackKind::Wisp {
                    let desired = (player_pos - attack.position).normalize_or(attack.direction);
                    let angle = attack
                        .direction
                        .angle_to(desired)
                        .clamp(-WISP_TURN_RATE * dt, WISP_TURN_RATE * dt);
                    attack.direction =
                        rotate(attack.direction, angle).normalize_or(attack.direction);
                }
                let step = attack.direction * attack.speed * dt;
                let moved = grid.move_and_collide(Vec2::splat(1.0), attack.position, step);
                attack.position = moved;
                transform.translation.x = moved.x;
                transform.translation.y = moved.y;

                let hits_player =
                    circles_overlap(attack.position, attack.radius, player_pos, player_half.x);
                let hits_owner =
                    attack_hits_owner(&attack, attack.position, attack.radius, &boss_map);

                if hits_player && !hits_owner {
                    damage_player(
                        &mut health,
                        &mut movement,
                        attack.position,
                        player_pos,
                        attack.damage,
                        &mut sfx,
                        &mut died,
                    );
                    status.apply_reversal(if attack.phase >= 2 {
                        REVERSAL_DURATION_P2
                    } else {
                        REVERSAL_DURATION
                    });
                    sfx.write(PlaySfx(Sfx::MadnessApply));
                    commands.entity(entity).despawn();
                } else if attack.remaining <= 0.0 {
                    commands.entity(entity).despawn();
                }
            }
            AttackKind::Amalgam => {
                if !attack.impacted {
                    attack.impacted = true;
                    let clear = !line_blocked(&grid, attack.position, player_pos);
                    let hits_player = clear
                        && circles_overlap(
                            attack.position,
                            attack.radius,
                            player_pos,
                            player_half.x,
                        );
                    let hits_owner =
                        attack_hits_owner(&attack, attack.position, attack.radius, &boss_map);

                    if hits_player && !hits_owner {
                        damage_player(
                            &mut health,
                            &mut movement,
                            attack.position,
                            player_pos,
                            attack.damage,
                            &mut sfx,
                            &mut died,
                        );
                    }
                    sfx.write(PlaySfx(Sfx::AmalgamExplode));
                }
                if attack.remaining <= 0.0 {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

/// Acid pools and extra debris left by a phase-2 slam.
fn spawn_slam_aftermath(commands: &mut Commands, attack: &BossAttack) {
    for index in 0..SLAM_ACID_POOLS_P2 {
        let angle = (index as f32) * std::f32::consts::TAU / SLAM_ACID_POOLS_P2 as f32;
        let offset = Vec2::new(angle.cos(), angle.sin()) * attack.radius;
        let pool = BossAttack::new(AttackKind::AcidPool, attack.position + offset)
            .with_radius(ACID_POOL_RADIUS * HITBOX_SHRINK)
            .with_damage(ACID_POOL_DAMAGE)
            .with_lifetime(ACID_POOL_LIFE)
            .with_phase(attack.phase);
        spawn_attack(commands, pool, 0.0);
    }
}

/// True when a solid tile sits between two points, used so a pillar blocks the
/// Amalgamation blast.
fn line_blocked(grid: &SolidGrid, from: Vec2, to: Vec2) -> bool {
    let delta = to - from;
    let steps = (delta.length() / (grid.tile_size() * 0.5)).ceil() as usize;
    for step in 1..steps {
        let t = step as f32 / steps as f32;
        if grid.is_solid_at(from + delta * t) {
            return true;
        }
    }
    false
}

/// Drives a boss through its tailings charge, smearing caustic trail behind it.
pub fn tick_surge_chargers(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<SolidGrid>,
    player: Query<&Transform, With<Player>>,
    mut bosses: Query<(Entity, &Boss, &mut Transform, &mut SurgeCharger), Without<Player>>,
) {
    let dt = time.delta_secs();
    let player_pos = player.single().map(|t| t.translation.truncate()).ok();

    for (entity, boss, mut transform, mut charger) in bosses.iter_mut() {
        let half = Vec2::splat(boss.id.size() * 0.5);
        let from = transform.translation.truncate();

        if charger.windup > 0.0 {
            let was_winding = charger.windup;
            charger.windup -= dt;
            if was_winding > 0.0 && charger.windup <= 0.0 {
                // Snap the aim onto the player as the charge begins, biased
                // away from the weaker downward animations.
                if let Some(player_pos) = player_pos {
                    charger.direction = crate::systems::boss_animation::bias_surge_direction(
                        (player_pos - from).normalize_or(charger.direction),
                    );
                }
            }
            continue;
        }

        if charger.active > 0.0 {
            charger.active -= dt;
            charger.direction =
                rotate(charger.direction, charger.curve * dt).normalize_or(charger.direction);

            let moved = grid.move_and_collide(half, from, charger.direction * charger.speed * dt);
            transform.translation.x = moved.x;
            transform.translation.y = moved.y;

            charger.trail_timer -= dt;
            if charger.trail_timer <= 0.0 {
                charger.trail_timer = 0.06;
                let enraged = charger.charges_left > 1;
                let width = if enraged {
                    SURGE_TRAIL_WIDTH_P2
                } else {
                    SURGE_TRAIL_WIDTH
                };
                let trail = BossAttack::new(AttackKind::SurgeTrail, moved)
                    .owned_by(entity)
                    .with_phase(if enraged { 2 } else { 1 })
                    .with_radius(width * 0.5 * HITBOX_SHRINK)
                    .with_damage(SURGE_TRAIL_DAMAGE)
                    .with_lifetime(SURGE_TRAIL_LIFE);
                spawn_attack(&mut commands, trail, 0.0);
            }
            continue;
        }

        // The charge finished; run another in phase 2 or detach.
        charger.charges_left = charger.charges_left.saturating_sub(1);
        if charger.charges_left > 0 {
            charger.windup = SURGE_REWINDUP;
            charger.active = SURGE_DURATION;
            charger.trail_timer = 0.0;
        } else {
            commands.entity(entity).remove::<SurgeCharger>();
        }
    }
}

/// Removes every attack entity a fight created, on leaving the arena.
pub fn cleanup_boss_encounter(
    mut commands: Commands,
    armed: Query<Entity, With<BossEncounterEntity>>,
) {
    for entity in armed.iter() {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::effect_sprite::EffectKind;

    #[test]
    fn a_line_with_a_wall_between_is_blocked() {
        let grid = crate::levels::grid::grid_from_rows(&["#####", "#...#", "#####"], 32.0);
        let bottom = grid.tile_center(1, 1);
        let top = grid.tile_center(3, 1);
        assert!(!line_blocked(&grid, bottom, top));

        let wall = grid.tile_center(2, 0);
        assert!(line_blocked(&grid, bottom, wall));
    }

    #[test]
    fn rotating_a_direction_by_zero_is_identity() {
        assert_eq!(rotate(Vec2::X, 0.0), Vec2::X);
    }

    #[test]
    fn rotating_a_direction_by_a_quarter_turn() {
        let turned = rotate(Vec2::X, std::f32::consts::FRAC_PI_2);
        assert!((turned - Vec2::Y).length() < 0.0001);
    }

    #[test]
    fn a_wave_hitbox_matches_its_drawn_wall() {
        let half = Vec2::splat(8.0);
        let position = Vec2::ZERO;
        let forward = Vec2::X;
        // The wall is wide across the travel: a player beside it is caught.
        assert!(wave_hits(position, forward, Vec2::new(0.0, 60.0), half));
        // But it is shallow along the travel: just past the front face is safe.
        assert!(wave_hits(position, forward, Vec2::new(28.0, 0.0), half));
        assert!(!wave_hits(position, forward, Vec2::new(-60.0, 0.0), half));
        assert!(!wave_hits(position, forward, Vec2::new(60.0, 0.0), half));
    }

    #[test]
    fn a_spawned_falling_attack_carries_a_shadow_telegraph() {
        let mut app = App::new();
        app.add_systems(Update, |mut commands: Commands| {
            let attack = BossAttack::new(AttackKind::Debris, Vec2::new(3.0, 4.0)).with_windup(1.0);
            spawn_attack(&mut commands, attack, 0.0);
        });
        app.update();

        let world = app.world_mut();
        let mut query = world.query::<(&BossAttack, &EffectSprite)>();
        let (attack, effect) = query.iter(world).next().expect("attack spawned");
        assert_eq!(attack.kind, AttackKind::Debris);
        assert_eq!(
            effect.kind,
            EffectKind::DebrisShadow,
            "a falling attack telegraphs with the ground shadow"
        );
    }

    #[test]
    fn every_attack_has_its_own_effect() {
        let kinds = [
            AttackKind::SurgeTrail,
            AttackKind::AcidPool,
            AttackKind::Slam,
            AttackKind::Debris,
            AttackKind::MercuryPool,
            AttackKind::Decoy,
            AttackKind::Wave,
            AttackKind::Spray,
            AttackKind::Wisp,
            AttackKind::Amalgam,
        ];
        let mut effects: Vec<EffectKind> = kinds.iter().map(|kind| kind.effect()).collect();
        effects.sort_unstable_by_key(|kind| *kind as u8);
        let count = effects.len();
        effects.dedup();
        assert_eq!(count, effects.len(), "two attacks share one effect");
    }
}
