use crate::components::boss::{AttackKind, Boss, BossAttack, BossEncounterEntity, SurgeCharger};
use crate::components::player::{Health, Movement, PLAYER_SIZE, Player};
use crate::constants::*;
use crate::events::{DamageDealt, PlaySfx, PlayerDied, Sfx};
use crate::levels::grid::SolidGrid;
use crate::resources::player_status::PlayerStatus;
use crate::systems::combat::{hurt_player, swing_hits};
use crate::utils::targeting::circles_overlap;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

/// Spawns one attack entity with its placeholder sprite. Shared by the pattern
/// spawner and the surge charger so attacks always carry the same plumbing.
pub fn spawn_attack(
    commands: &mut Commands,
    attack: BossAttack,
    color: Color,
    size: Vec2,
    angle: f32,
) -> Entity {
    let position = attack.position;
    commands
        .spawn((
            attack,
            BossEncounterEntity,
            Sprite {
                color,
                custom_size: Some(size),
                ..default()
            },
            Transform::from_xyz(position.x, position.y, 1.0)
                .with_rotation(Quat::from_rotation_z(angle)),
            Name::new("Boss Attack"),
        ))
        .id()
}

/// The placeholder colour for each attack.
pub fn attack_color(kind: AttackKind) -> Color {
    match kind {
        AttackKind::SurgeTrail => Color::srgba(0.62, 0.85, 0.32, 0.55),
        AttackKind::AcidPool => Color::srgba(0.55, 0.8, 0.25, 0.5),
        AttackKind::Slam => Color::srgba(0.9, 0.32, 0.22, 0.45),
        AttackKind::Debris => Color::srgba(0.16, 0.16, 0.22, 0.7),
        AttackKind::MercuryPool => Color::srgba(0.72, 0.86, 0.92, 0.5),
        AttackKind::Decoy => Color::srgba(0.78, 0.9, 0.96, 0.9),
        AttackKind::Wave => Color::srgba(0.7, 0.9, 1.0, 0.7),
        AttackKind::Spray => Color::srgba(0.82, 0.92, 1.0, 0.85),
        AttackKind::Wisp => Color::srgba(0.86, 0.96, 1.0, 0.9),
        AttackKind::Amalgam => Color::srgba(0.7, 0.25, 0.42, 0.5),
    }
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

/// Draws the telegraph an attack shows while it winds up.
///
/// Ground attacks keep their landing marker (and the debris shadow tracks its
/// target); thrown attacks are hidden so only the boss's clip reads as a tell.
fn windup_visual(sprite: &mut Sprite, attack: &BossAttack, transform: &mut Transform) {
    match attack.kind {
        AttackKind::Debris => {
            transform.translation.x = attack.target.x;
            transform.translation.y = attack.target.y;
        }
        AttackKind::Wave | AttackKind::Spray | AttackKind::Wisp => {
            sprite.color.set_alpha(0.0);
        }
        AttackKind::Amalgam => {
            let progress = 1.0 - (attack.windup / AMALGAMATION_CHANNEL).clamp(0.0, 1.0);
            sprite.color = Color::srgba(0.7, 0.25, 0.42, 0.25 + 0.4 * progress);
        }
        _ => {}
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

/// Moves, ages and resolves every spawned boss attack.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn tick_boss_attacks(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<SolidGrid>,
    mut status: ResMut<PlayerStatus>,
    mut attacks: Query<
        (Entity, &mut BossAttack, &mut Transform, &mut Sprite),
        (Without<Player>, Without<Boss>),
    >,
    boss_transforms: Query<&Transform, (With<Boss>, Without<Player>)>,
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

    for (entity, mut attack, mut transform, mut sprite) in attacks.iter_mut() {
        attack.hit_cooldown = (attack.hit_cooldown - dt).max(0.0);
        attack.aux_timer = (attack.aux_timer - dt).max(0.0);
        attack.remaining -= dt;

        // A winding-up attack only telegraphs: no motion and no damage yet.
        if !attack.armed {
            attack.windup = (attack.windup - dt).max(0.0);
            if attack.windup > 0.0 {
                windup_visual(&mut sprite, &attack, &mut transform);
                continue;
            }
            attack.armed = true;
            // Thrown attacks leave from the boss's current spot and snap onto
            // the player as they release.
            if attack.kind.aims_at_player() {
                if let Some(owner) = attack.owner
                    && let Ok(owner_transform) = boss_transforms.get(owner)
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
                if circles_overlap(attack.position, radius, player_pos, player_half.x) {
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
                fade_hazard(&mut sprite, &attack);
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
                if attack.hit_cooldown <= 0.0
                    && circles_overlap(attack.position, attack.radius, player_pos, player_half.x)
                {
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
                fade_hazard(&mut sprite, &attack);
                if attack.remaining <= 0.0 {
                    commands.entity(entity).despawn();
                }
            }
            AttackKind::Debris => {
                if !attack.impacted {
                    attack.impacted = true;
                    if circles_overlap(attack.target, attack.radius, player_pos, player_half.x) {
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
                            .with_radius(SURGE_TRAIL_WIDTH_P2)
                            .with_damage(attack.damage * 0.3)
                            .with_lifetime(SURGE_TRAIL_LIFE)
                            .with_phase(attack.phase);
                        spawn_attack(
                            &mut commands,
                            mist,
                            attack_color(AttackKind::SurgeTrail),
                            Vec2::splat(SURGE_TRAIL_WIDTH_P2),
                            0.0,
                        );
                    }
                }
                commands.entity(entity).despawn();
            }
            AttackKind::Decoy => {
                if attack.remaining <= 0.0 {
                    commands.entity(entity).despawn();
                }
            }
            AttackKind::Wave => {
                sprite.color = attack_color(AttackKind::Wave);
                let step = attack.direction * attack.speed * dt;
                let moved = grid.move_and_collide(Vec2::splat(1.0), attack.position, step);
                attack.position = moved;
                transform.translation.x = moved.x;
                transform.translation.y = moved.y;

                if attack.hit_cooldown <= 0.0
                    && wave_hits(attack.position, attack.direction, player_pos, player_half)
                {
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
                        .with_radius(MERCURY_POOL_RADIUS)
                        .with_damage(MERCURY_POOL_DAMAGE)
                        .with_lifetime(MERCURY_POOL_LIFE)
                        .with_phase(attack.phase);
                    spawn_attack(
                        &mut commands,
                        pool,
                        attack_color(AttackKind::MercuryPool),
                        Vec2::splat(MERCURY_POOL_RADIUS * 2.0),
                        0.0,
                    );
                }
                if attack.remaining <= 0.0 {
                    commands.entity(entity).despawn();
                }
            }
            AttackKind::Spray | AttackKind::Wisp => {
                sprite.color = attack_color(attack.kind);
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

                if circles_overlap(attack.position, attack.radius, player_pos, player_half.x) {
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
                    if clear
                        && circles_overlap(
                            attack.position,
                            attack.radius,
                            player_pos,
                            player_half.x,
                        )
                    {
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

/// Fades a lingering hazard as it runs out.
fn fade_hazard(sprite: &mut Sprite, attack: &BossAttack) {
    let life = (attack.remaining / attack.total).clamp(0.0, 1.0);
    let base = attack_color(attack.kind).to_srgba();
    sprite.color = Color::srgba(
        base.red,
        base.green,
        base.blue,
        base.alpha * (0.25 + 0.75 * life),
    );
}

/// Acid pools and extra debris left by a phase-2 slam.
fn spawn_slam_aftermath(commands: &mut Commands, attack: &BossAttack) {
    for index in 0..SLAM_ACID_POOLS_P2 {
        let angle = (index as f32) * std::f32::consts::TAU / SLAM_ACID_POOLS_P2 as f32;
        let offset = Vec2::new(angle.cos(), angle.sin()) * attack.radius;
        let pool = BossAttack::new(AttackKind::AcidPool, attack.position + offset)
            .with_radius(ACID_POOL_RADIUS)
            .with_damage(ACID_POOL_DAMAGE)
            .with_lifetime(ACID_POOL_LIFE)
            .with_phase(attack.phase);
        spawn_attack(
            commands,
            pool,
            attack_color(AttackKind::AcidPool),
            Vec2::splat(ACID_POOL_RADIUS * 2.0),
            0.0,
        );
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
                let width = if charger.charges_left > 1 {
                    SURGE_TRAIL_WIDTH_P2
                } else {
                    SURGE_TRAIL_WIDTH
                };
                let trail = BossAttack::new(AttackKind::SurgeTrail, moved)
                    .owned_by(entity)
                    .with_radius(width * 0.5)
                    .with_damage(SURGE_TRAIL_DAMAGE)
                    .with_lifetime(SURGE_TRAIL_LIFE);
                spawn_attack(
                    &mut commands,
                    trail,
                    attack_color(AttackKind::SurgeTrail),
                    Vec2::splat(width),
                    0.0,
                );
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
    fn every_attack_has_a_distinct_placeholder_colour() {
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
        for (index, first) in kinds.iter().enumerate() {
            for second in kinds.iter().skip(index + 1) {
                assert_ne!(
                    attack_color(*first).to_srgba(),
                    attack_color(*second).to_srgba(),
                    "{first:?} and {second:?} share a colour"
                );
            }
        }
    }
}
