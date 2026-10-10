use crate::components::attack::{AttackType, AttackVisual};
use crate::components::boss::{Boss, BossAttack};
use crate::components::player::{Movement, Player};
use crate::components::targetable::{Decoy, Targetable};
use crate::constants::{BOSS_HURTBOX_SCALE, DECOY_SIZE, LIGHT_WIDTH};
use crate::events::{DamageDealt, HitConfirm, PlaySfx, Sfx};
use crate::materials::attack_effect::AttackEffectMaterial;
use crate::resources::inventory_panel::InventoryPanel;
use crate::resources::lock_on::LockOn;
use crate::resources::player_attack_state::PlayerAttackState;
use crate::resources::run_data::PlayerGear;
use crate::states::Phase;
use crate::systems::attack_effect::{AttackEffectAccess, spawn_swing};
use crate::systems::boss_attacks::{mercury_pool, spawn_attack};
use crate::systems::combat::{damage_after_armor, swing_hits};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::sprite_render::MeshMaterial2d;

/// Turns light and heavy input into a swing in front of the player.
///
/// The swing faces the locked-on target when there is one, otherwise the nearest
/// targetable, and finally the last movement direction so a keyboard-only player
/// can still aim. Damage is routed through [`crate::events::DamageDealt`] so
/// single bosses, dual halves and decoys all resolve in one place.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn player_attack(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    mut state: ResMut<PlayerAttackState>,
    gear: Res<PlayerGear>,
    inventory: Res<InventoryPanel>,
    lock: Res<LockOn>,
    player: Query<(&Transform, &Movement), With<Player>>,
    targetables: Query<
        (
            Entity,
            &Transform,
            Option<&Boss>,
            Option<&Decoy>,
            Option<&BossAttack>,
        ),
        With<Targetable>,
    >,
    mut damage_events: MessageWriter<DamageDealt>,
    mut hit_events: MessageWriter<HitConfirm>,
    mut sfx: MessageWriter<PlaySfx>,
    mut effects: AttackEffectAccess,
    phase: Phase,
) {
    state.tick(time.delta_secs());

    let Ok((transform, movement)) = player.single() else {
        return;
    };
    let origin = transform.translation.truncate();

    // Face the locked target, then the nearest targetable, then WASD.
    if phase.is_boss_fight() {
        let locked_pos = lock
            .target
            .and_then(|entity| targetables.get(entity).ok())
            .map(|(_, transform, _, _, _)| transform.translation.truncate());
        let nearest = nearest_targetable_position(&targetables, origin);
        if let Some(target) = locked_pos.or(nearest) {
            state.facing = (target - origin).normalize_or(Vec2::X);
        }
    } else if movement.input_direction != Vec2::ZERO {
        state.facing = movement.input_direction.normalize();
    }

    if !phase.is_boss_fight() || inventory.open {
        return;
    }
    if state.is_rooted() {
        return;
    }

    let light = keys.just_pressed(KeyCode::KeyQ) || mouse.just_pressed(MouseButton::Left);
    let heavy = keys.just_pressed(KeyCode::KeyE) || mouse.just_pressed(MouseButton::Right);

    let attack = if heavy && state.is_ready(AttackType::Heavy) {
        Some(AttackType::Heavy)
    } else if light && state.is_ready(AttackType::Light) {
        Some(AttackType::Light)
    } else {
        None
    };
    let Some(attack) = attack else {
        return;
    };

    let stats = gear.stats();
    let (reach, width, raw_damage) = match attack {
        AttackType::Light => (
            stats.light_reach,
            LIGHT_WIDTH,
            stats.weapon_damage * attack.multiplier(),
        ),
        AttackType::Heavy => (
            stats.heavy_reach,
            stats.heavy_width,
            stats.weapon_damage * attack.multiplier(),
        ),
    };

    let facing = state.facing;
    for (entity, target_transform, boss, decoy, decoy_attack) in targetables.iter() {
        let target_pos = target_transform.translation.truncate();
        let half = if let Some(boss) = boss {
            Vec2::splat(boss.id.size() * 0.5 * BOSS_HURTBOX_SCALE)
        } else {
            Vec2::splat(DECOY_SIZE * 0.5 * BOSS_HURTBOX_SCALE)
        };
        if !swing_hits(origin, facing, reach, width, target_pos, half) {
            continue;
        }

        if decoy.is_some() {
            hit_events.write(HitConfirm {
                target: entity,
                position: target_pos,
            });
            sfx.write(PlaySfx(Sfx::DecoyPop));
            // Popping a decoy leaves a mercury puddle that keeps hurting
            // anyone standing in it, instead of bursting for instant splash
            // damage. The decoy carries the phase it was spawned in, so an
            // enraged fight leaves a bigger, nastier pool.
            let pool_phase = decoy_attack.map_or(1, |attack| attack.phase);
            let mut pool = mercury_pool(target_pos, pool_phase);
            if let Some(owner) = decoy_attack.and_then(|attack| attack.owner) {
                pool = pool.owned_by(owner);
            }
            spawn_attack(&mut commands, pool, 0.0);
            commands.entity(entity).despawn();
            continue;
        }

        if let Some(boss) = boss {
            let damage = damage_after_armor(raw_damage, 0.0);
            damage_events.write(DamageDealt {
                target: entity,
                amount: damage,
                raw: raw_damage,
            });
            hit_events.write(HitConfirm {
                target: entity,
                position: impact_point(origin, target_pos, reach),
            });
            log::debug!("{} hit {:?} for {damage}", attack.label(), boss.id);
        }
    }

    spawn_swing(
        &mut commands,
        &mut effects,
        origin,
        facing,
        reach,
        width,
        attack,
        gear.weapon.map(|piece| piece.set),
    );
    state.begin(attack);
    sfx.write(PlaySfx(match attack {
        AttackType::Light => Sfx::PlayerSwingLight,
        AttackType::Heavy => Sfx::PlayerSwingHeavy,
    }));
}

/// The nearest targetable's world position to `origin`.
#[allow(clippy::type_complexity)]
fn nearest_targetable_position(
    targetables: &Query<
        (
            Entity,
            &Transform,
            Option<&Boss>,
            Option<&Decoy>,
            Option<&BossAttack>,
        ),
        With<Targetable>,
    >,
    origin: Vec2,
) -> Option<Vec2> {
    targetables
        .iter()
        .map(|(_, transform, _, _, _)| transform.translation.truncate())
        .min_by(|a, b| {
            a.distance_squared(origin)
                .total_cmp(&b.distance_squared(origin))
        })
}

/// Where the swing tip meets the target, used to place the impact spark.
fn impact_point(origin: Vec2, target_pos: Vec2, reach: f32) -> Vec2 {
    origin + (target_pos - origin).clamp_length_max(reach)
}

/// Despawns swing graphics once their lifetime runs out, keeping the shader's
/// animation clock in step with the swing.
pub fn tick_attack_visuals(
    mut commands: Commands,
    time: Res<Time>,
    mut effects: AttackEffectAccess,
    mut visuals: Query<(
        Entity,
        &mut AttackVisual,
        Option<&MeshMaterial2d<AttackEffectMaterial>>,
    )>,
) {
    let dt = time.delta_secs();
    let elapsed = time.elapsed_secs();
    for (entity, mut visual, material) in visuals.iter_mut() {
        visual.remaining -= dt;
        let progress = if visual.total > 0.0 {
            1.0 - (visual.remaining / visual.total).clamp(0.0, 1.0)
        } else {
            1.0
        };
        if let Some(material) = material {
            effects.set_progress(&material.0, progress, elapsed);
        }
        if visual.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::boss::{AttackKind, BossId, Dying};
    use crate::components::player::{Health, Movement};
    use crate::events::PlayerDied;
    use crate::resources::boss_encounter::SharedBossHealth;
    use crate::resources::player_status::PlayerStatus;
    use crate::states::{DayPhase, GameState};
    use crate::systems::boss_damage::{apply_boss_damage, tick_dying_bosses};
    use bevy::ecs::message::MessageReader;
    use bevy::state::app::StatesPlugin;
    use bevy::transform::TransformPlugin;

    #[derive(Resource, Default)]
    struct CapturedDamage(Vec<DamageDealt>);

    #[derive(Resource, Default)]
    struct CapturedDefeats(Vec<crate::events::BossDefeated>);

    #[derive(Resource, Default)]
    struct CapturedHits(Vec<HitConfirm>);

    fn capture_damage(
        mut captured: ResMut<CapturedDamage>,
        mut reader: MessageReader<DamageDealt>,
    ) {
        for event in reader.read() {
            captured.0.push(*event);
        }
    }

    fn capture_defeats(
        mut captured: ResMut<CapturedDefeats>,
        mut reader: MessageReader<crate::events::BossDefeated>,
    ) {
        for event in reader.read() {
            captured.0.push(*event);
        }
    }

    fn capture_hits(mut captured: ResMut<CapturedHits>, mut reader: MessageReader<HitConfirm>) {
        for event in reader.read() {
            captured.0.push(*event);
        }
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<PlayerAttackState>()
            .init_resource::<PlayerGear>()
            .init_resource::<InventoryPanel>()
            .init_resource::<LockOn>()
            .init_resource::<PlayerStatus>()
            .init_resource::<SharedBossHealth>()
            .init_resource::<crate::resources::boss_encounter::BossCoordinator>()
            .init_resource::<CapturedDamage>()
            .init_resource::<CapturedDefeats>()
            .init_resource::<CapturedHits>()
            .add_message::<PlayerDied>()
            .add_message::<PlaySfx>()
            .add_message::<DamageDealt>()
            .add_message::<HitConfirm>()
            .add_message::<crate::events::BossDefeated>()
            .add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_systems(Update, (player_attack, tick_attack_visuals).chain())
            .add_systems(Update, apply_boss_damage.after(player_attack))
            .add_systems(Update, tick_dying_bosses.after(apply_boss_damage))
            .add_systems(
                Update,
                (capture_damage, capture_defeats, capture_hits).after(tick_dying_bosses),
            );

        app.world_mut().spawn((
            Player,
            Movement::default(),
            Health::new(100.0, 0.0),
            Transform::from_xyz(0.0, 0.0, 1.0),
        ));

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossFight);
        app.update();
        app
    }

    fn spawn_boss(app: &mut App, id: BossId, pos: Vec2) -> Entity {
        app.world_mut()
            .spawn((
                Boss::new_at(id, pos),
                Targetable,
                Transform::from_xyz(pos.x, pos.y, 0.0),
            ))
            .id()
    }

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear_just_pressed(key);
    }

    fn boss_health(app: &App, entity: Entity) -> Option<f32> {
        app.world().get::<Boss>(entity).map(|boss| boss.health)
    }

    #[test]
    fn a_light_swing_damages_a_boss_in_front() {
        let mut app = setup_app();
        let boss = spawn_boss(&mut app, BossId::BossA, Vec2::new(50.0, 0.0));
        let full = crate::constants::BOSS_A_HEALTH;

        press(&mut app, KeyCode::KeyQ);

        assert_eq!(
            boss_health(&app, boss),
            Some(full - crate::constants::UNARMED_DAMAGE)
        );
        assert_eq!(app.world().resource::<CapturedDamage>().0.len(), 1);
    }

    #[test]
    fn a_hit_confirms_with_an_impact_position() {
        let mut app = setup_app();
        let boss = spawn_boss(&mut app, BossId::BossA, Vec2::new(50.0, 0.0));

        press(&mut app, KeyCode::KeyQ);

        let hits = &app.world().resource::<CapturedHits>().0;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].target, boss);
        assert!(hits[0].position.x > 0.0);
    }

    #[test]
    fn a_heavy_swing_hits_harder_than_a_light_one() {
        let mut app = setup_app();
        let boss = spawn_boss(&mut app, BossId::BossA, Vec2::new(50.0, 0.0));

        press(&mut app, KeyCode::KeyE);

        let expected = crate::constants::BOSS_A_HEALTH - crate::constants::UNARMED_DAMAGE * 1.8;
        assert_eq!(boss_health(&app, boss), Some(expected));
    }

    #[test]
    fn a_boss_out_of_reach_is_not_hit() {
        let mut app = setup_app();
        let boss = spawn_boss(&mut app, BossId::BossA, Vec2::new(500.0, 0.0));
        let full = crate::constants::BOSS_A_HEALTH;

        press(&mut app, KeyCode::KeyQ);

        assert_eq!(boss_health(&app, boss), Some(full));
        assert!(app.world().resource::<CapturedDamage>().0.is_empty());
    }

    #[test]
    fn auto_face_targets_the_nearest_boss() {
        let mut app = setup_app();
        let near = spawn_boss(&mut app, BossId::BossA, Vec2::new(40.0, 0.0));
        let behind = spawn_boss(&mut app, BossId::BossB, Vec2::new(-300.0, 0.0));

        press(&mut app, KeyCode::KeyQ);

        assert!(boss_health(&app, near) < Some(crate::constants::BOSS_A_HEALTH));
        assert_eq!(
            boss_health(&app, behind),
            Some(crate::constants::BOSS_B_HEALTH)
        );
    }

    #[test]
    fn a_locked_target_overrides_the_nearest() {
        let mut app = setup_app();
        let near = spawn_boss(&mut app, BossId::BossA, Vec2::new(300.0, 0.0));
        let chosen = spawn_boss(&mut app, BossId::BossA, Vec2::new(-40.0, 0.0));
        app.world_mut().resource_mut::<LockOn>().set(chosen);

        press(&mut app, KeyCode::KeyQ);

        assert!(boss_health(&app, chosen) < Some(crate::constants::BOSS_A_HEALTH));
        assert_eq!(
            boss_health(&app, near),
            Some(crate::constants::BOSS_A_HEALTH)
        );
    }

    #[test]
    fn a_swing_roots_the_player() {
        let mut app = setup_app();
        spawn_boss(&mut app, BossId::BossA, Vec2::new(50.0, 0.0));
        press(&mut app, KeyCode::KeyQ);
        assert!(app.world().resource::<PlayerAttackState>().is_rooted());
    }

    #[test]
    fn attacking_outside_a_boss_fight_does_nothing() {
        let mut app = setup_app();
        let boss = spawn_boss(&mut app, BossId::BossA, Vec2::new(50.0, 0.0));
        let full = crate::constants::BOSS_A_HEALTH;
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);
        app.update();

        press(&mut app, KeyCode::KeyQ);

        assert_eq!(boss_health(&app, boss), Some(full));
    }

    #[test]
    fn a_lethal_blow_defeats_and_despawns_the_boss() {
        let mut app = setup_app();
        let boss = spawn_boss(&mut app, BossId::BossA, Vec2::new(50.0, 0.0));
        app.world_mut().get_mut::<Boss>(boss).unwrap().health = 1.0;

        press(&mut app, KeyCode::KeyQ);

        // The lethal blow starts the death clip; the boss is not gone yet.
        assert!(app.world().get::<Boss>(boss).is_some());
        assert!(app.world().get::<Dying>(boss).is_some());

        app.world_mut().get_mut::<Dying>(boss).unwrap().remaining = 0.0;
        app.update();

        assert!(app.world().get::<Boss>(boss).is_none());
        let defeats = &app.world().resource::<CapturedDefeats>().0;
        assert_eq!(defeats.len(), 1);
        assert_eq!(defeats[0].0, BossId::BossA);
    }

    #[test]
    fn striking_a_decoy_destroys_it_and_leaves_a_mercury_pool() {
        let mut app = setup_app();
        let decoy = app
            .world_mut()
            .spawn((Targetable, Decoy, Transform::from_xyz(50.0, 0.0, 0.0)))
            .id();

        press(&mut app, KeyCode::KeyQ);

        assert!(app.world().get::<Decoy>(decoy).is_none());
        let pool = find_mercury_pool(&mut app).expect("a mercury pool is left behind");
        assert_eq!(pool.position, Vec2::new(50.0, 0.0));
        assert_eq!(pool.phase, 1);
    }

    #[test]
    fn popping_an_enraged_decoy_leaves_a_bigger_pool() {
        let mut app = setup_app();
        app.world_mut().spawn((
            Targetable,
            Decoy,
            BossAttack::new(AttackKind::Decoy, Vec2::new(50.0, 0.0)).with_phase(2),
            Transform::from_xyz(50.0, 0.0, 0.0),
        ));

        press(&mut app, KeyCode::KeyQ);

        let pool = find_mercury_pool(&mut app).expect("a mercury pool is left behind");
        assert_eq!(pool.phase, 2);
        assert!(
            pool.radius > crate::constants::MERCURY_POOL_RADIUS * crate::constants::HITBOX_SHRINK,
            "an enraged pool should be wider than a phase-1 one"
        );
    }

    fn find_mercury_pool(app: &mut App) -> Option<BossAttack> {
        app.world_mut()
            .query::<&BossAttack>()
            .iter(app.world())
            .find(|attack| attack.kind == AttackKind::MercuryPool)
            .cloned()
    }

    #[test]
    fn an_expired_visual_is_despawned() {
        let mut app = setup_app();
        let visual = app
            .world_mut()
            .spawn((
                AttackVisual {
                    attack_type: AttackType::Light,
                    remaining: 0.0,
                    total: 0.2,
                    hitbox: Vec2::new(55.0, 28.0),
                },
                Transform::default(),
            ))
            .id();

        app.update();

        assert!(app.world().get::<AttackVisual>(visual).is_none());
    }

    #[test]
    fn a_light_swing_spawns_a_visual_carrying_its_hitbox() {
        let mut app = setup_app();
        spawn_boss(&mut app, BossId::BossA, Vec2::new(50.0, 0.0));

        press(&mut app, KeyCode::KeyQ);

        let visual = app
            .world_mut()
            .query_filtered::<&AttackVisual, With<AttackVisual>>()
            .iter(app.world())
            .next()
            .copied()
            .expect("a swing visual is spawned");
        assert_eq!(visual.attack_type, AttackType::Light);
        assert_eq!(
            visual.hitbox,
            Vec2::new(
                crate::constants::UNARMED_LIGHT_REACH,
                crate::constants::LIGHT_WIDTH
            )
        );
    }
}
