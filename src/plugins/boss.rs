use crate::components::boss::{BOSS_SIZE, Boss, BossSpawnMarker, boss_for_level};
use crate::events::{BossDefeated, BossPhaseChanged, DamageDealt, HitConfirm, PlayerDied};
use crate::resources::level::{ActiveLevel, BossSpawn, LevelSet};
use crate::resources::player_attack_state::PlayerAttackState;
use crate::states::{DayPhase, Phase};
use crate::systems::boss_ai::boss_ai;
use crate::systems::combat::{player_death_check, tick_combat_timers};
use crate::systems::hit_effects::{spawn_hit_sparks, tick_hit_sparks};
use crate::systems::player_attack::{player_attack, tick_attack_visuals};
use bevy::prelude::*;

pub struct BossPlugin;

impl Plugin for BossPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerAttackState>()
            .add_message::<BossDefeated>()
            .add_message::<PlayerDied>()
            .add_message::<BossPhaseChanged>()
            .add_message::<DamageDealt>()
            .add_message::<HitConfirm>()
            .add_systems(Update, spawn_boss.after(LevelSet::Load))
            .add_systems(
                Update,
                (
                    player_attack,
                    tick_attack_visuals,
                    tick_combat_timers,
                    player_death_check,
                    spawn_hit_sparks,
                    tick_hit_sparks,
                ),
            )
            .add_systems(FixedUpdate, boss_ai)
            .add_systems(OnExit(DayPhase::BossFight), despawn_boss);
    }
}

/// Spawns the arena's boss once the room has actually loaded, so it lands on the
/// level's `b` marker instead of the previous room's origin.
fn spawn_boss(
    mut commands: Commands,
    active: Res<ActiveLevel>,
    spawn: Res<BossSpawn>,
    phase: Phase,
    bosses: Query<(), With<Boss>>,
) {
    if !phase.is_boss_fight() || !bosses.is_empty() {
        return;
    }
    let Some(id) = boss_for_level(active.id) else {
        return;
    };

    commands.spawn((
        Boss::new_at(id, spawn.position),
        BossSpawnMarker,
        Sprite {
            color: id.color(),
            custom_size: Some(Vec2::splat(BOSS_SIZE)),
            ..default()
        },
        Transform::from_xyz(spawn.position.x, spawn.position.y, 0.0),
        Name::new(id.label()),
    ));
}

fn despawn_boss(mut commands: Commands, bosses: Query<Entity, With<BossSpawnMarker>>) {
    for entity in bosses.iter() {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::boss::BossId;
    use crate::components::player::Movement;
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
        // Enough frames for the level swap and the boss spawn to land.
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
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].boss_id, BossId::BossA);
        assert_eq!(changes[0].new_phase, 2);
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
