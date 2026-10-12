use crate::components::boss::{Boss, BossBrain, BossId, BossSpawnMarker, DualRole, boss_for_level};
use crate::components::boss_animation::{BossAnimation, BossPack};
use crate::components::targetable::Targetable;
use crate::events::{
    BossAttackStarted, BossDefeated, BossPhaseChanged, DamageDealt, HitConfirm, PlaySfx, PlayerDied,
};
use crate::materials::attack_effect::AttackEffectMaterial;
use crate::resources::boss_encounter::{BossCoordinator, SharedBossHealth};
use crate::resources::boss_rng::BossRng;
use crate::resources::boss_sprite::BossSpriteAssets;
use crate::resources::circle_art::CircleArt;
use crate::resources::difficulty::Difficulty;
use crate::resources::drop_rng::DropRng;
use crate::resources::effect_sprite::EffectSpriteAssets;
use crate::resources::infinite_mode::{InfiniteMode, RESPAWN_SECONDS};
use crate::resources::inventory::Inventory;
use crate::resources::level::{ActiveLevel, BossSpawn, LevelSet};
use crate::resources::player_attack_state::PlayerAttackState;
use crate::resources::player_status::PlayerStatus;
use crate::states::{DayPhase, GameState, Phase};
use crate::systems::attack_effect::AttackEffectAssets;
use crate::systems::boss_ai::boss_ai;
use crate::systems::boss_animation::{animate_boss_sprite, drive_boss_animation};
use crate::systems::boss_attacks::{
    cleanup_boss_encounter, tick_boss_attacks, tick_surge_chargers,
};
use crate::systems::boss_damage::{apply_boss_damage, tick_dying_bosses};
use crate::systems::boss_movement::boss_movement;
use crate::systems::boss_patterns::{resolve_pending_blinks, spawn_pattern_attacks};
use crate::systems::combat::{player_death_check, tick_combat_timers};
use crate::systems::damage_numbers::{spawn_damage_numbers, tick_damage_numbers};
use crate::systems::effect_sprite::{animate_effect_sprites, sync_attack_effects};
use crate::systems::hit_effects::{spawn_hit_sparks, tick_hit_sparks};
use crate::systems::hit_pause::world_unfrozen;
use crate::systems::player_attack::{player_attack, tick_attack_visuals};
use crate::systems::player_status::tick_player_status;
use crate::systems::target_indicator::update_target_indicator;
use bevy::asset::AssetServer;
use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;

pub struct BossPlugin;

impl Plugin for BossPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerAttackState>()
            .init_resource::<BossRng>()
            .init_resource::<BossCoordinator>()
            .init_resource::<SharedBossHealth>()
            .init_resource::<InfiniteMode>()
            // The Infinite Mode death handler touches these; initialising them
            // here keeps BossPlugin runnable on its own (e.g. in unit tests).
            .init_resource::<Inventory>()
            .init_resource::<DropRng>()
            .init_resource::<Difficulty>()
            .init_resource::<PlayerStatus>()
            .init_resource::<BossSpriteAssets>()
            .init_resource::<EffectSpriteAssets>()
            .init_resource::<AttackEffectAssets>()
            .init_resource::<CircleArt>()
            .add_message::<BossDefeated>()
            .add_message::<PlayerDied>()
            .add_message::<BossPhaseChanged>()
            .add_message::<DamageDealt>()
            .add_message::<HitConfirm>()
            .add_message::<BossAttackStarted>()
            .add_message::<PlaySfx>()
            .add_message::<crate::events::PlayLoopSfx>()
            .add_systems(Update, spawn_boss.after(LevelSet::Load))
            .add_systems(
                FixedUpdate,
                (
                    boss_ai,
                    spawn_pattern_attacks,
                    tick_surge_chargers,
                    boss_movement,
                )
                    .chain()
                    .run_if(world_unfrozen),
            )
            .add_systems(
                Update,
                (
                    tick_player_status,
                    player_attack,
                    tick_attack_visuals,
                    tick_combat_timers,
                    player_death_check,
                    spawn_hit_sparks,
                    tick_hit_sparks,
                    update_target_indicator,
                    tick_boss_attacks,
                    resolve_pending_blinks,
                    sync_attack_effects,
                    tick_damage_numbers,
                    spawn_damage_numbers,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                apply_boss_damage
                    .after(player_attack)
                    .after(tick_boss_attacks),
            )
            .add_systems(Update, (drive_boss_animation, animate_boss_sprite).chain())
            .add_systems(Update, animate_effect_sprites.after(sync_attack_effects))
            .add_systems(Update, tick_dying_bosses.after(apply_boss_damage))
            .add_systems(
                Update,
                track_infinite_deaths
                    .after(tick_dying_bosses)
                    .after(spawn_boss),
            )
            .add_systems(Update, tick_infinite_respawn.after(track_infinite_deaths))
            .add_systems(
                OnExit(DayPhase::BossFight),
                (despawn_boss, cleanup_boss_encounter, end_encounter),
            )
            // A permadeath death leaves `Playing` for `Victory` without passing
            // through `Result`, and `DayPhase`'s exit schedule does not run when
            // its parent state goes away. Sweep the encounter here too so the
            // boss never follows the player back to the farm.
            .add_systems(
                OnExit(GameState::Playing),
                (despawn_boss, cleanup_boss_encounter, end_encounter),
            );

        // A custom 2D material needs the asset infrastructure that only the
        // real renderer brings; headless tests run without it, so register the
        // material plugin only when it exists.
        if app.world().get_resource::<AssetServer>().is_some() {
            crate::materials::attack_effect::register(app);
            app.add_plugins(Material2dPlugin::<AttackEffectMaterial>::default());
        }
    }
}

/// The level, spawn markers and difficulty a boss spawn reads, bundled so the
/// signature stays readable.
#[derive(bevy::ecs::system::SystemParam)]
struct BossSpawnContext<'w> {
    active: Res<'w, ActiveLevel>,
    spawn: Res<'w, BossSpawn>,
    difficulty: Res<'w, Difficulty>,
}

/// Spawns the arena's boss(es) once the room has loaded, so they land on the
/// level's `b` marker(s). The dual arena spawns one entity per half.
fn spawn_boss(
    mut commands: Commands,
    context: BossSpawnContext,
    phase: Phase,
    bosses: Query<(), With<Boss>>,
    mut shared: ResMut<SharedBossHealth>,
    mut coordinator: ResMut<BossCoordinator>,
    mut infinite: ResMut<InfiniteMode>,
) {
    if !phase.is_boss_fight() {
        return;
    }

    let health_multiplier = context.difficulty.boss_health_multiplier();
    let damage_multiplier = context.difficulty.boss_damage_multiplier();

    if infinite.active {
        // Only the opening pair is set up here; the respawn timers own the
        // bosses afterwards, so a frame where both happen to be down does not
        // re-field them on top of their scheduled returns.
        if infinite.spawned {
            return;
        }
        infinite.spawned = true;
        // Infinite Mode reuses the dual arena but fields the Excavator and the
        // Quicksilver as independent singles: no shared pool, no coordination.
        coordinator.end();
        shared.reset();
        let mut spots = context.spawn.all();
        if spots.len() < 2 {
            spots.push(spots.first().copied().unwrap_or(Vec2::ZERO) + Vec2::new(96.0, 0.0));
        }
        for (role, position) in [DualRole::Excavator, DualRole::Quicksilver]
            .into_iter()
            .zip(spots.iter().copied())
        {
            let id = role.id();
            spawn_one(
                &mut commands,
                id,
                id.size(),
                id.color(),
                position,
                None,
                health_multiplier * infinite.health_multiplier(role),
                damage_multiplier,
            );
        }
        return;
    }

    if !bosses.is_empty() {
        return;
    }

    let Some(id) = boss_for_level(context.active.id) else {
        return;
    };

    if id == BossId::Dual {
        shared.reset_scaled(health_multiplier);
        coordinator.begin();
        let mut spots = context.spawn.all();
        if spots.len() < 2 {
            spots.push(spots.first().copied().unwrap_or(Vec2::ZERO) + Vec2::new(96.0, 0.0));
        }
        for (role, position) in [DualRole::Excavator, DualRole::Quicksilver]
            .into_iter()
            .zip(spots.iter().copied())
        {
            spawn_one(
                &mut commands,
                id,
                role.id().size(),
                role.id().color(),
                position,
                Some(role),
                health_multiplier,
                damage_multiplier,
            );
        }
        return;
    }

    coordinator.end();
    shared.reset();
    let position = context
        .spawn
        .all()
        .first()
        .copied()
        .unwrap_or(context.spawn.position);
    spawn_one(
        &mut commands,
        id,
        id.size(),
        id.color(),
        position,
        None,
        health_multiplier,
        damage_multiplier,
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_one(
    commands: &mut Commands,
    id: crate::components::boss::BossId,
    size: f32,
    color: Color,
    position: Vec2,
    role: Option<DualRole>,
    health_multiplier: f32,
    damage_multiplier: f32,
) {
    let pack = role
        .map(BossPack::from_role)
        .unwrap_or_else(|| BossPack::from_id(id));
    let mut entity = commands.spawn((
        Boss::new_scaled(id, position, health_multiplier, damage_multiplier),
        BossBrain::with_opening_grace(),
        BossAnimation::new(pack, position),
        Targetable,
        BossSpawnMarker,
        Sprite {
            color,
            custom_size: Some(Vec2::splat(size)),
            ..default()
        },
        Transform::from_xyz(position.x, position.y, 0.0),
        Name::new(role.map(DualRole::label).unwrap_or(id.label())),
    ));
    if let Some(role) = role {
        entity.insert(role);
    }
}

fn despawn_boss(mut commands: Commands, bosses: Query<Entity, With<BossSpawnMarker>>) {
    for entity in bosses.iter() {
        commands.entity(entity).despawn();
    }
}

/// Clears the encounter's run-scoped state when the fight ends.
fn end_encounter(
    mut coordinator: ResMut<BossCoordinator>,
    mut shared: ResMut<SharedBossHealth>,
    mut status: ResMut<PlayerStatus>,
    mut infinite: ResMut<InfiniteMode>,
) {
    coordinator.end();
    shared.reset();
    status.clear();
    infinite.end();
}

/// In Infinite Mode a felled boss drops its materials and starts its respawn
/// timer. Its kill points come from the shared [`crate::plugins::points`]
/// system, which reads the same defeat message in every mode.
fn track_infinite_deaths(
    mut events: MessageReader<BossDefeated>,
    mut infinite: ResMut<InfiniteMode>,
    mut inventory: ResMut<Inventory>,
    mut drop_rng: ResMut<DropRng>,
) {
    if !infinite.active {
        return;
    }
    for event in events.read() {
        let role = match event.0 {
            BossId::BossA => DualRole::Excavator,
            BossId::BossB => DualRole::Quicksilver,
            _ => continue,
        };
        for (material, range) in event.0.material_drops() {
            let amount = drop_rng.roll(range.clone());
            inventory.add_material(*material, amount);
        }
        infinite.record_death(role);
        infinite.set_timer(role, Some(RESPAWN_SECONDS));
    }
}

/// Counts down each downed Infinite Mode boss's respawn timer and brings it back
/// at full health, tougher for every fall it has already taken.
fn tick_infinite_respawn(
    mut commands: Commands,
    time: Res<Time>,
    mut infinite: ResMut<InfiniteMode>,
    context: BossSpawnContext,
    phase: Phase,
) {
    if !infinite.active || !phase.is_boss_fight() {
        return;
    }
    let dt = time.delta_secs();
    let spots = context.spawn.all();
    for (role, index) in [
        (DualRole::Excavator, 0usize),
        (DualRole::Quicksilver, 1usize),
    ] {
        let Some(mut remaining) = infinite.timer(role) else {
            continue;
        };
        remaining -= dt;
        if remaining > 0.0 {
            infinite.set_timer(role, Some(remaining));
            continue;
        }
        let id = role.id();
        let position = spots.get(index).copied().unwrap_or(context.spawn.position);
        spawn_one(
            &mut commands,
            id,
            id.size(),
            id.color(),
            position,
            None,
            context.difficulty.boss_health_multiplier() * infinite.health_multiplier(role),
            context.difficulty.boss_damage_multiplier(),
        );
        infinite.set_timer(role, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::boss::BossId;
    use crate::components::player::{Health, Movement};
    use crate::plugins::level::LevelPlugin;
    use crate::resources::inventory_panel::InventoryPanel;
    use crate::resources::level::{LevelRequest, build_level, prop_position};
    use crate::resources::run_data::PlayerGear;
    use crate::states::GameState;
    use bevy::ecs::message::MessageReader;
    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;
    use bevy::transform::TransformPlugin;
    use std::time::Duration;

    #[derive(Resource, Default)]
    struct PhaseChanges(Vec<BossPhaseChanged>);

    fn capture_phase_changes(
        mut captured: ResMut<PhaseChanges>,
        mut reader: MessageReader<BossPhaseChanged>,
    ) {
        for event in reader.read() {
            captured.0.push(*event);
        }
    }

    #[derive(Resource, Default)]
    struct AttacksStarted(usize);

    fn capture_attacks(
        mut captured: ResMut<AttacksStarted>,
        mut reader: MessageReader<BossAttackStarted>,
    ) {
        for _ in reader.read() {
            captured.0 += 1;
        }
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<PlayerGear>()
            .init_resource::<InventoryPanel>()
            .init_resource::<PhaseChanges>()
            .init_resource::<AttacksStarted>()
            .add_plugins((
                MinimalPlugins,
                TransformPlugin,
                StatesPlugin,
                LevelPlugin,
                BossPlugin,
            ))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                16,
            )))
            .add_systems(Update, (capture_phase_changes, capture_attacks));

        app.world_mut().spawn((
            crate::components::player::Player,
            Movement::default(),
            Health::new(100.0, 0.0),
            Transform::from_xyz(0.0, 0.0, 1.0),
        ));
        app
    }

    fn enter_fight(app: &mut App, level: crate::levels::LevelId) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        app.world_mut().resource_mut::<LevelRequest>().0 = Some(level);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossFight);
        for _ in 0..3 {
            app.update();
        }
    }

    fn boss_entity(app: &mut App) -> Entity {
        app.world_mut()
            .query_filtered::<Entity, With<Boss>>()
            .iter(app.world())
            .next()
            .expect("boss spawned")
    }

    #[test]
    fn a_boss_spawns_on_the_arena_marker_with_its_own_health() {
        let mut app = setup_app();
        enter_fight(&mut app, crate::levels::LevelId::ArenaA);

        let entity = boss_entity(&mut app);
        let boss = *app.world().get::<Boss>(entity).unwrap();
        assert_eq!(boss.id, BossId::BossA);
        assert_eq!(boss.health, BossId::BossA.max_health());
        assert_eq!(boss.max_health, BossId::BossA.max_health());
        assert_eq!(boss.phase, 1);

        let (def, grid) = build_level(crate::levels::LevelId::ArenaA);
        let expected = prop_position(&grid, &def, crate::levels::PropKind::BossSpawn).unwrap();
        assert_eq!(boss.home, [expected.x, expected.y]);
    }

    #[test]
    fn the_dual_arena_spawns_both_halves_with_shared_health() {
        let mut app = setup_app();
        enter_fight(&mut app, crate::levels::LevelId::ArenaDual);

        let count = app
            .world_mut()
            .query_filtered::<Entity, With<Boss>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 2);
        assert!(app.world().resource::<BossCoordinator>().active);
    }

    #[test]
    fn a_boss_spawns_with_difficulty_scaled_health() {
        let mut app = setup_app();
        *app.world_mut().resource_mut::<Difficulty>() = Difficulty::Hard;
        enter_fight(&mut app, crate::levels::LevelId::ArenaA);

        let entity = boss_entity(&mut app);
        let boss = *app.world().get::<Boss>(entity).unwrap();
        let expected = BossId::BossA.max_health() * Difficulty::Hard.boss_health_multiplier();
        assert!(
            (boss.max_health - expected).abs() < 0.01,
            "health scales with difficulty"
        );
        assert_eq!(boss.health, boss.max_health);
        assert_eq!(
            boss.damage_multiplier,
            Difficulty::Hard.boss_damage_multiplier(),
            "the attack scale is captured at spawn"
        );
    }

    #[test]
    fn the_dual_pool_scales_with_difficulty_too() {
        let mut app = setup_app();
        *app.world_mut().resource_mut::<Difficulty>() = Difficulty::Apocalypse;
        enter_fight(&mut app, crate::levels::LevelId::ArenaDual);

        let shared = app.world().resource::<SharedBossHealth>();
        let expected =
            crate::constants::DUAL_BOSS_HEALTH * Difficulty::Apocalypse.boss_health_multiplier();
        assert!((shared.max - expected).abs() < 0.01);
    }

    #[test]
    fn a_boss_holds_off_until_the_opening_grace_elapses() {
        let mut app = setup_app();
        enter_fight(&mut app, crate::levels::LevelId::ArenaA);

        let entity = boss_entity(&mut app);
        let brain = app.world().get::<BossBrain>(entity).unwrap();
        assert!(
            brain.cooldown > 0.0,
            "the boss should still be waiting out its opening buffer"
        );
        assert!(
            brain.current.is_none(),
            "no pattern picked during the grace"
        );
        assert_eq!(app.world().resource::<AttacksStarted>().0, 0);

        // Spend the buffer and let the brain make its first pick.
        app.world_mut()
            .get_mut::<BossBrain>(entity)
            .unwrap()
            .cooldown = 0.0;
        for _ in 0..4 {
            app.update();
        }

        assert!(
            app.world()
                .get::<BossBrain>(entity)
                .unwrap()
                .current
                .is_some(),
            "the boss should engage once the buffer is spent"
        );
        assert!(app.world().resource::<AttacksStarted>().0 > 0);
    }

    #[test]
    fn a_dual_fight_waits_out_the_opening_grace_too() {
        let mut app = setup_app();
        enter_fight(&mut app, crate::levels::LevelId::ArenaDual);

        let coordinator = *app.world().resource::<BossCoordinator>();
        assert!(coordinator.active);
        assert!(
            coordinator.next_combo > 0.0,
            "the dual fight should not open with an immediate combo"
        );
        assert_eq!(app.world().resource::<AttacksStarted>().0, 0);
    }

    #[test]
    fn dropping_to_half_health_enrages_the_boss_and_announces_it() {
        let mut app = setup_app();
        enter_fight(&mut app, crate::levels::LevelId::ArenaA);
        let entity = boss_entity(&mut app);
        app.world_mut().get_mut::<Boss>(entity).unwrap().health = 1.0;

        for _ in 0..6 {
            app.update();
        }

        assert_eq!(app.world().get::<Boss>(entity).unwrap().phase, 2);
        let changes = &app.world().resource::<PhaseChanges>().0;
        assert!(!changes.is_empty());
        assert!(changes.iter().any(|change| change.new_phase == 2));
    }

    #[test]
    fn leaving_the_fight_removes_the_boss() {
        let mut app = setup_app();
        enter_fight(&mut app, crate::levels::LevelId::ArenaA);
        let entity = boss_entity(&mut app);
        assert!(app.world().get::<Boss>(entity).is_some());

        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Result);
        app.update();

        assert!(
            app.world_mut()
                .query_filtered::<Entity, With<Boss>>()
                .iter(app.world())
                .next()
                .is_none()
        );
    }

    #[test]
    fn leaving_play_from_a_fight_removes_the_boss() {
        let mut app = setup_app();
        enter_fight(&mut app, crate::levels::LevelId::ArenaA);
        let entity = boss_entity(&mut app);
        assert!(app.world().get::<Boss>(entity).is_some());

        // Permadeath death jumps straight to `Victory`, skipping `Result`.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        for _ in 0..2 {
            app.update();
        }

        assert!(
            app.world_mut()
                .query_filtered::<Entity, With<Boss>>()
                .iter(app.world())
                .next()
                .is_none(),
            "the boss is swept when play ends, however it ends"
        );
    }

    #[test]
    fn infinite_mode_spawns_two_independent_bosses() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<InfiniteMode>().begin();
        enter_fight(&mut app, crate::levels::LevelId::ArenaDual);

        let ids: Vec<BossId> = app
            .world_mut()
            .query_filtered::<&Boss, With<Boss>>()
            .iter(app.world())
            .map(|boss| boss.id)
            .collect();
        assert_eq!(ids.len(), 2, "both bosses are fielded");
        assert!(ids.contains(&BossId::BossA));
        assert!(ids.contains(&BossId::BossB));
        assert!(
            !app.world().resource::<BossCoordinator>().active,
            "endurance fights are not coordinated"
        );

        // Each boss carries its own full health, not a shared pool.
        let healths: Vec<f32> = app
            .world_mut()
            .query_filtered::<&Boss, With<Boss>>()
            .iter(app.world())
            .map(|boss| boss.health)
            .collect();
        assert!(healths.iter().all(|health| *health > 0.0));
        assert_ne!(healths[0], healths[1], "the bosses differ in health");
    }

    #[test]
    fn a_felled_infinite_boss_schedules_a_tougher_respawn() {
        use crate::components::boss::Dying;

        let mut app = setup_app();
        app.world_mut().resource_mut::<InfiniteMode>().begin();
        enter_fight(&mut app, crate::levels::LevelId::ArenaDual);

        let excavator = app
            .world_mut()
            .query_filtered::<Entity, With<Boss>>()
            .iter(app.world())
            .find(|entity| app.world().get::<Boss>(*entity).unwrap().id == BossId::BossA)
            .expect("the excavator is spawned");

        app.world_mut().write_message(DamageDealt {
            target: excavator,
            amount: 1_000_000.0,
            raw: 1_000_000.0,
        });
        for _ in 0..3 {
            app.update();
        }
        if let Some(mut dying) = app.world_mut().get_mut::<Dying>(excavator) {
            dying.remaining = 0.0;
        }
        for _ in 0..3 {
            app.update();
        }

        assert!(
            app.world().get::<Boss>(excavator).is_none(),
            "the felled boss leaves the arena"
        );
        let infinite = *app.world().resource::<InfiniteMode>();
        assert_eq!(infinite.deaths(DualRole::Excavator), 1);
        assert!(
            infinite.timer(DualRole::Excavator).is_some(),
            "a respawn is scheduled"
        );

        // Fast-forward the timer and let the respawn run.
        app.world_mut()
            .resource_mut::<InfiniteMode>()
            .set_timer(DualRole::Excavator, Some(0.0));
        for _ in 0..3 {
            app.update();
        }

        let returned = app
            .world_mut()
            .query_filtered::<&Boss, With<Boss>>()
            .iter(app.world())
            .find(|boss| boss.id == BossId::BossA)
            .expect("the boss climbs back up");
        let base = BossId::BossA.max_health();
        assert!(
            returned.max_health > base,
            "the return is tougher: {} vs {base}",
            returned.max_health
        );
        assert_eq!(returned.health, returned.max_health, "back at full health");
    }
}
