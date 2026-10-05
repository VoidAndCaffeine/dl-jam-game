use crate::components::boss::{BOSS_SIZE, Boss, BossSpawnMarker, boss_for_level};
use crate::components::player::{PLAYER_SIZE, Player};
use crate::events::{BossDefeated, InteractionEvent, PlayerDied};
use crate::plugins::interaction::{HIGHLIGHT_Z, HighlightMarker, Interactable};
use crate::resources::level::{ActiveLevel, BossSpawn, LevelSet};
use crate::states::{DayPhase, Phase};
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::prelude::*;

pub struct BossPlugin;

impl Plugin for BossPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BossDefeated>()
            .add_message::<PlayerDied>()
            .add_systems(Update, spawn_boss.after(LevelSet::Load))
            .add_systems(Update, boss_combat)
            .add_systems(Update, boss_contact_damage)
            .add_systems(OnExit(DayPhase::BossFight), despawn_boss);
    }
}

/// Spawns the arena's placeholder boss once the room has actually loaded, so it
/// lands on the level's `b` marker instead of the previous room's origin.
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

    commands
        .spawn((
            Boss::new(id),
            BossSpawnMarker,
            Interactable::new(),
            Sprite {
                color: id.color(),
                custom_size: Some(Vec2::splat(BOSS_SIZE)),
                ..default()
            },
            Transform::from_xyz(spawn.position.x, spawn.position.y, 0.0),
            Name::new(id.label()),
        ))
        .with_children(|parent| {
            parent.spawn((
                HighlightMarker,
                Sprite {
                    color: Color::srgba(1.0, 1.0, 0.0, 0.5),
                    custom_size: Some(Vec2::splat(BOSS_SIZE * 1.15)),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, HIGHLIGHT_Z),
                Visibility::Hidden,
                Name::new("Highlight"),
            ));
        });
}

fn despawn_boss(mut commands: Commands, bosses: Query<Entity, With<BossSpawnMarker>>) {
    for entity in bosses.iter() {
        commands.entity(entity).despawn();
    }
}

/// Attacks the boss when the player interacts with it. Placeholder bosses die in
/// one hit, so this is where a fight currently ends.
fn boss_combat(
    mut events: MessageReader<InteractionEvent>,
    mut bosses: Query<&mut Boss>,
    mut defeated: MessageWriter<BossDefeated>,
    mut commands: Commands,
    phase: Phase,
) {
    // Drain every event even outside the fight so farm clicks are never replayed
    // against a boss once its fight begins.
    for event in events.read() {
        if !phase.is_boss_fight() {
            continue;
        }
        let Ok(mut boss) = bosses.get_mut(event.entity) else {
            continue;
        };
        let id = boss.id;
        let max_health = boss.max_health;
        if boss.damage(max_health) {
            defeated.write(BossDefeated(id));
            commands.entity(event.entity).despawn();
        }
    }
}

/// Placeholder contact damage: walking into a boss kills the player, which is
/// how the defeat path is exercised until real attack patterns exist.
fn boss_contact_damage(
    mut died: MessageWriter<PlayerDied>,
    player: Query<&Transform, With<Player>>,
    bosses: Query<&Transform, With<Boss>>,
    phase: Phase,
) {
    if !phase.is_boss_fight() {
        return;
    }
    let Ok(player) = player.single() else {
        return;
    };
    let player_pos = player.translation.truncate();
    let reach = (PLAYER_SIZE + BOSS_SIZE) * 0.5;

    for boss in bosses.iter() {
        let boss_pos = boss.translation.truncate();
        if (player_pos.x - boss_pos.x).abs() < reach && (player_pos.y - boss_pos.y).abs() < reach {
            died.write(PlayerDied);
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::boss::BossId;
    use crate::events::InteractionType;
    use crate::plugins::level::LevelPlugin;
    use crate::resources::level::{LevelRequest, build_level, prop_position};
    use crate::states::GameState;
    use bevy::ecs::message::MessageReader;
    use bevy::state::app::StatesPlugin;
    use bevy::transform::TransformPlugin;

    #[derive(Resource, Default)]
    struct Captured(Vec<BossDefeated>);

    #[derive(Resource, Default)]
    struct Deaths(Vec<PlayerDied>);

    fn capture_defeats(mut captured: ResMut<Captured>, mut reader: MessageReader<BossDefeated>) {
        for event in reader.read() {
            captured.0.push(*event);
        }
    }

    fn capture_deaths(mut captured: ResMut<Deaths>, mut reader: MessageReader<PlayerDied>) {
        for event in reader.read() {
            captured.0.push(*event);
        }
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Captured>()
            .init_resource::<Deaths>()
            .init_resource::<crate::resources::crafting_menu::CraftingMenu>()
            .init_resource::<crate::resources::inventory_panel::InventoryPanel>()
            .add_plugins((
                MinimalPlugins,
                TransformPlugin,
                StatesPlugin,
                LevelPlugin,
                BossPlugin,
            ))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_message::<InteractionEvent>()
            .add_systems(
                Update,
                (capture_defeats, capture_deaths)
                    .after(boss_combat)
                    .after(spawn_boss),
            );

        app.world_mut()
            .spawn((Player, Transform::from_xyz(0.0, 0.0, 1.0)));
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

    fn move_player(app: &mut App, pos: Vec2) {
        let mut query = app
            .world_mut()
            .query_filtered::<&mut Transform, With<Player>>();
        let mut transform = query.single_mut(app.world_mut()).unwrap();
        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
    }

    #[test]
    fn a_boss_spawns_on_the_arena_marker() {
        let mut app = setup_app();
        enter_fight(&mut app, crate::levels::LevelId::ArenaA);

        let entity = boss_entity(&mut app);
        let boss = *app.world().get::<Boss>(entity).unwrap();
        assert_eq!(boss.id, BossId::BossA);
        assert_eq!(boss.health, crate::components::boss::BOSS_HEALTH);

        let (def, grid) = build_level(crate::levels::LevelId::ArenaA);
        let expected = prop_position(&grid, &def, crate::levels::PropKind::BossSpawn).unwrap();
        let actual = app
            .world()
            .get::<Transform>(entity)
            .unwrap()
            .translation
            .truncate();
        assert_eq!(actual, expected);
    }

    #[test]
    fn interacting_with_the_boss_defeats_it() {
        let mut app = setup_app();
        enter_fight(&mut app, crate::levels::LevelId::ArenaA);
        let entity = boss_entity(&mut app);

        app.world_mut().write_message(InteractionEvent {
            entity,
            interaction_type: InteractionType::BossArena,
        });
        app.update();

        assert!(app.world().get::<Boss>(entity).is_none(), "boss despawns");
        let captured = app.world().resource::<Captured>();
        assert_eq!(captured.0.len(), 1);
        assert_eq!(captured.0[0].0, BossId::BossA);
    }

    #[test]
    fn touching_the_boss_kills_the_player() {
        let mut app = setup_app();
        enter_fight(&mut app, crate::levels::LevelId::ArenaB);
        let entity = boss_entity(&mut app);
        let pos = app
            .world()
            .get::<Transform>(entity)
            .unwrap()
            .translation
            .truncate();
        move_player(&mut app, pos);
        app.update();

        assert_eq!(app.world().resource::<Deaths>().0.len(), 1);
    }

    #[test]
    fn a_distant_player_survives() {
        let mut app = setup_app();
        enter_fight(&mut app, crate::levels::LevelId::ArenaB);
        move_player(&mut app, Vec2::new(-1000.0, -1000.0));
        app.update();

        assert!(app.world().resource::<Deaths>().0.is_empty());
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
