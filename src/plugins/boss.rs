use crate::components::boss::{Boss, BossBrain, BossSpawnMarker, DualRole, boss_for_level};
use crate::components::boss_animation::{BossAnimation, BossPack};
use crate::components::targetable::Targetable;
use crate::events::{
    BossAttackStarted, BossDefeated, BossPhaseChanged, DamageDealt, HitConfirm, PlaySfx, PlayerDied,
};
use crate::resources::boss_encounter::{BossCoordinator, SharedBossHealth};
use crate::resources::boss_rng::BossRng;
use crate::resources::boss_sprite::BossSpriteAssets;
use crate::resources::level::{ActiveLevel, BossSpawn, LevelSet};
use crate::resources::lock_on::LockOn;
use crate::resources::player_attack_state::PlayerAttackState;
use crate::resources::player_status::PlayerStatus;
use crate::states::{DayPhase, Phase};
use crate::systems::boss_ai::boss_ai;
use crate::systems::boss_animation::{animate_boss_sprite, drive_boss_animation};
use crate::systems::boss_attacks::{
    cleanup_boss_encounter, tick_boss_attacks, tick_surge_chargers,
};
use crate::systems::boss_damage::{apply_boss_damage, tick_dying_bosses};
use crate::systems::boss_patterns::spawn_pattern_attacks;
use crate::systems::combat::{player_death_check, tick_combat_timers};
use crate::systems::hit_effects::{spawn_hit_sparks, tick_hit_sparks};
use crate::systems::lock_on::{cycle_lock_on, update_lock_on_visuals};
use crate::systems::player_attack::{player_attack, tick_attack_visuals};
use crate::systems::player_status::tick_player_status;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

pub struct BossPlugin;

impl Plugin for BossPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerAttackState>()
            .init_resource::<BossRng>()
            .init_resource::<BossCoordinator>()
            .init_resource::<SharedBossHealth>()
            .init_resource::<LockOn>()
            .init_resource::<PlayerStatus>()
            .init_resource::<BossSpriteAssets>()
            .add_message::<BossDefeated>()
            .add_message::<PlayerDied>()
            .add_message::<BossPhaseChanged>()
            .add_message::<DamageDealt>()
            .add_message::<HitConfirm>()
            .add_message::<BossAttackStarted>()
            .add_message::<PlaySfx>()
            .add_message::<MouseWheel>()
            .add_systems(Update, spawn_boss.after(LevelSet::Load))
            .add_systems(
                FixedUpdate,
                (boss_ai, spawn_pattern_attacks, tick_surge_chargers).chain(),
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
                    cycle_lock_on,
                    update_lock_on_visuals,
                    tick_boss_attacks,
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
            .add_systems(Update, tick_dying_bosses.after(apply_boss_damage))
            .add_systems(
                OnExit(DayPhase::BossFight),
                (despawn_boss, cleanup_boss_encounter, end_encounter),
            );
    }
}

/// Spawns the arena's boss(es) once the room has loaded, so they land on the
/// level's `b` marker(s). The dual arena spawns one entity per half.
fn spawn_boss(
    mut commands: Commands,
    active: Res<ActiveLevel>,
    spawn: Res<BossSpawn>,
    phase: Phase,
    bosses: Query<(), With<Boss>>,
    mut shared: ResMut<SharedBossHealth>,
    mut coordinator: ResMut<BossCoordinator>,
) {
    if !phase.is_boss_fight() || !bosses.is_empty() {
        return;
    }
    let Some(id) = boss_for_level(active.id) else {
        return;
    };

    if id == crate::components::boss::BossId::Dual {
        shared.reset();
        coordinator.begin();
        let mut spots = spawn.all();
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
            );
        }
        return;
    }

    coordinator.end();
    shared.reset();
    let position = spawn.all().first().copied().unwrap_or(spawn.position);
    spawn_one(&mut commands, id, id.size(), id.color(), position, None);
}

fn spawn_one(
    commands: &mut Commands,
    id: crate::components::boss::BossId,
    size: f32,
    color: Color,
    position: Vec2,
    role: Option<DualRole>,
) {
    let pack = role
        .map(BossPack::from_role)
        .unwrap_or_else(|| BossPack::from_id(id));
    let mut entity = commands.spawn((
        Boss::new_at(id, position),
        BossBrain::default(),
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
    mut lock: ResMut<LockOn>,
    mut status: ResMut<PlayerStatus>,
) {
    coordinator.end();
    shared.reset();
    lock.clear();
    status.clear();
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

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<PlayerGear>()
            .init_resource::<InventoryPanel>()
            .init_resource::<PhaseChanges>()
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
            .add_systems(Update, capture_phase_changes);

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
}
