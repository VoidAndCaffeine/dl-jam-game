use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use bevy::transform::TransformPlugin;
use dl_jam::GamePlugin;
use dl_jam::components::boss::{Boss, BossId, Dying};
use dl_jam::components::player::{Health, Player};
use dl_jam::components::pot::CropType;
use dl_jam::events::{InteractionEvent, InteractionType};
use dl_jam::levels::LevelId;
use dl_jam::plugins::interaction::BossArenaEntry;
use dl_jam::resources::boss_progress::BossProgress;
use dl_jam::resources::boss_select::BossSelectMenu;
use dl_jam::resources::farm::CropUnlocks;
use dl_jam::resources::level::ActiveLevel;
use dl_jam::resources::player_attack_state::PlayerAttackState;
use dl_jam::resources::player_status::PlayerStatus;
use dl_jam::states::DayPhase;

fn setup_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin, GamePlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(16),
        ));
    app
}

fn enter_playing(app: &mut App) {
    for _ in 0..3 {
        app.update();
    }
}

fn step(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

/// Runs frames until the day phase reaches `target`, so the extra loading frames
/// a scene change now costs do not have to be counted by hand.
fn step_until(app: &mut App, target: DayPhase) {
    for _ in 0..40 {
        if phase(app) == target {
            return;
        }
        app.update();
    }
    panic!("never reached {target:?}; stuck in {:?}", phase(app));
}

fn tap(app: &mut App, key: KeyCode) {
    {
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.release(key);
        input.press(key);
    }
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .clear_just_pressed(key);
    app.update();
}

fn phase(app: &App) -> DayPhase {
    app.world().resource::<State<DayPhase>>().get().clone()
}

fn active(app: &App) -> ActiveLevel {
    app.world().resource::<ActiveLevel>().clone()
}

fn menu(app: &App) -> BossSelectMenu {
    app.world().resource::<BossSelectMenu>().clone()
}

fn first_of<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .iter(app.world())
        .next()
        .expect("expected at least one")
}

fn count<T: Component>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .iter(app.world())
        .count()
}

/// Weakens the live boss, stands the player in front of it and lands one light
/// swing, exercising the real attack, damage and defeat pipeline.
fn defeat_current_boss(app: &mut App) {
    let boss = first_of::<Boss>(app);
    app.world_mut().get_mut::<Boss>(boss).unwrap().health = 1.0;
    let boss_pos = app
        .world()
        .get::<Transform>(boss)
        .unwrap()
        .translation
        .truncate();

    {
        let mut query = app
            .world_mut()
            .query_filtered::<&mut Transform, With<Player>>();
        let mut transform = query.single_mut(app.world_mut()).unwrap();
        transform.translation.x = boss_pos.x - 40.0;
        transform.translation.y = boss_pos.y;
    }

    {
        let mut state = app.world_mut().resource_mut::<PlayerAttackState>();
        state.attack_lock = 0.0;
        state.light_cooldown = 0.0;
        state.heavy_cooldown = 0.0;
        state.facing = Vec2::X;
    }
    tap(app, KeyCode::KeyQ);
    step(app, 3);

    // The lethal blow starts the boss's death clip; skip to the end of it so
    // the defeat fires without waiting out the animation.
    if let Some(mut dying) = app.world_mut().get_mut::<Dying>(boss) {
        dying.remaining = 0.0;
    }
    step(app, 3);
}

#[test]
fn the_arena_gate_opens_the_boss_select_menu() {
    let mut app = setup_app();
    enter_playing(&mut app);
    assert_eq!(active(&app).id, LevelId::Farm);

    let gate = first_of::<BossArenaEntry>(&mut app);
    app.world_mut().write_message(InteractionEvent {
        entity: gate,
        interaction_type: InteractionType::BossArena,
    });
    step_until(&mut app, DayPhase::BossSelect);

    assert!(menu(&app).open);
    assert_eq!(phase(&app), DayPhase::BossSelect);
    assert_eq!(
        active(&app).id,
        LevelId::Farm,
        "menu does not load an arena"
    );
}

#[test]
fn escaping_the_menu_returns_to_the_farm() {
    let mut app = setup_app();
    enter_playing(&mut app);
    let gate = first_of::<BossArenaEntry>(&mut app);
    app.world_mut().write_message(InteractionEvent {
        entity: gate,
        interaction_type: InteractionType::BossArena,
    });
    step_until(&mut app, DayPhase::BossSelect);
    assert_eq!(phase(&app), DayPhase::BossSelect);

    tap(&mut app, KeyCode::Escape);
    step_until(&mut app, DayPhase::Farming);

    assert!(!menu(&app).open);
    assert_eq!(phase(&app), DayPhase::Farming);
    assert_eq!(active(&app).id, LevelId::Farm);
}

#[test]
fn the_full_boss_loop_runs_from_farm_to_result_and_back() {
    let mut app = setup_app();
    enter_playing(&mut app);

    // Open the menu through the gate.
    let gate = first_of::<BossArenaEntry>(&mut app);
    app.world_mut().write_message(InteractionEvent {
        entity: gate,
        interaction_type: InteractionType::BossArena,
    });
    step_until(&mut app, DayPhase::BossSelect);
    assert_eq!(phase(&app), DayPhase::BossSelect);

    // Choose Boss A and confirm.
    tap(&mut app, KeyCode::Enter);
    assert!(menu(&app).confirmation_open);
    tap(&mut app, KeyCode::Enter);
    step_until(&mut app, DayPhase::BossFight);

    assert_eq!(phase(&app), DayPhase::BossFight);
    assert_eq!(active(&app).id, LevelId::ArenaA);
    assert_eq!(count::<Boss>(&mut app), 1);

    // Land a lethal swing on the boss.
    defeat_current_boss(&mut app);

    assert_eq!(phase(&app), DayPhase::Result);
    assert!(app.world().resource::<BossProgress>().boss_a);
    assert!(
        app.world()
            .resource::<CropUnlocks>()
            .is_unlocked(CropType::CropA)
    );

    // Leave the result screen: back to the farm, no boss left behind.
    tap(&mut app, KeyCode::Space);
    step_until(&mut app, DayPhase::Farming);

    assert_eq!(phase(&app), DayPhase::Farming);
    assert_eq!(active(&app).id, LevelId::Farm);
    assert_eq!(count::<Boss>(&mut app), 0);
}

#[test]
fn beating_both_single_bosses_unlocks_the_dual_boss() {
    let mut app = setup_app();
    enter_playing(&mut app);

    for boss_id in [BossId::BossA, BossId::BossB] {
        // Open the menu.
        let gate = first_of::<BossArenaEntry>(&mut app);
        app.world_mut().write_message(InteractionEvent {
            entity: gate,
            interaction_type: InteractionType::BossArena,
        });
        step_until(&mut app, DayPhase::BossSelect);

        // Select the next available boss by confirming the current highlight.
        if boss_id == BossId::BossB {
            tap(&mut app, KeyCode::ArrowDown);
        }
        tap(&mut app, KeyCode::Enter);
        tap(&mut app, KeyCode::Enter);
        step_until(&mut app, DayPhase::BossFight);

        defeat_current_boss(&mut app);

        assert_eq!(phase(&app), DayPhase::Result);

        tap(&mut app, KeyCode::Space);
        step_until(&mut app, DayPhase::Farming);
        assert_eq!(active(&app).id, LevelId::Farm);
    }

    let progress = app.world().resource::<BossProgress>();
    assert!(progress.dual_boss_unlocked);
}

#[test]
fn the_dual_boss_stays_locked_until_both_are_beaten() {
    let mut app = setup_app();
    enter_playing(&mut app);

    let gate = first_of::<BossArenaEntry>(&mut app);
    app.world_mut().write_message(InteractionEvent {
        entity: gate,
        interaction_type: InteractionType::BossArena,
    });
    step_until(&mut app, DayPhase::BossSelect);

    // Down twice stops on Boss B because the dual boss is locked.
    tap(&mut app, KeyCode::ArrowDown);
    assert_eq!(menu(&app).selected, 1);
    tap(&mut app, KeyCode::ArrowDown);
    assert_eq!(menu(&app).selected, 1);

    tap(&mut app, KeyCode::Enter);
    tap(&mut app, KeyCode::Enter);
    step_until(&mut app, DayPhase::BossFight);
    assert_eq!(active(&app).id, LevelId::ArenaB);
}

fn player_health(app: &mut App) -> Health {
    let mut query = app.world_mut().query_filtered::<&Health, With<Player>>();
    query.single(app.world()).unwrap().clone()
}

/// Opens the gate, walks `target_index` rows down and confirms, leaving the app
/// in a boss fight against that boss.
fn start_boss_fight(app: &mut App, target_index: usize) {
    assert_eq!(phase(app), DayPhase::Farming, "must start from the farm");
    let gate = first_of::<BossArenaEntry>(app);
    app.world_mut().write_message(InteractionEvent {
        entity: gate,
        interaction_type: InteractionType::BossArena,
    });
    step_until(app, DayPhase::BossSelect);
    assert_eq!(phase(app), DayPhase::BossSelect);

    for _ in 0..target_index {
        tap(app, KeyCode::ArrowDown);
    }
    tap(app, KeyCode::Enter);
    tap(app, KeyCode::Enter);
    step_until(app, DayPhase::BossFight);
    assert_eq!(phase(app), DayPhase::BossFight);
}

/// Empties the player's health and lets the real death pipeline run.
fn kill_player(app: &mut App) {
    {
        let mut query = app
            .world_mut()
            .query_filtered::<&mut Health, With<Player>>();
        let mut health = query.single_mut(app.world_mut()).unwrap();
        health.current = 0.0;
    }
    step(app, 3);
    assert_eq!(
        phase(app),
        DayPhase::Result,
        "death should reach the result"
    );
}

/// Dismisses the result screen and waits for the farm to come back.
fn return_to_farm(app: &mut App) {
    tap(app, KeyCode::Space);
    step_until(app, DayPhase::Farming);
    assert_eq!(phase(app), DayPhase::Farming);
    assert_eq!(active(app).id, LevelId::Farm);
}

#[test]
fn player_health_is_restored_after_a_defeat() {
    let mut app = setup_app();
    enter_playing(&mut app);
    start_boss_fight(&mut app, 0);

    kill_player(&mut app);
    return_to_farm(&mut app);

    let health = player_health(&mut app);
    assert_eq!(health.current, health.max);
    assert_eq!(health.iframe_remaining, 0.0);
}

#[test]
fn player_health_is_restored_after_a_victory() {
    let mut app = setup_app();
    enter_playing(&mut app);
    start_boss_fight(&mut app, 0);

    // Wound the player, then win the fight.
    {
        let mut query = app
            .world_mut()
            .query_filtered::<&mut Health, With<Player>>();
        let mut health = query.single_mut(app.world_mut()).unwrap();
        health.current = 12.0;
    }
    defeat_current_boss(&mut app);
    assert_eq!(phase(&app), DayPhase::Result);

    return_to_farm(&mut app);
    let health = player_health(&mut app);
    assert_eq!(health.current, health.max);
}

#[test]
fn a_defeat_does_not_carry_into_the_next_boss_attempt() {
    let mut app = setup_app();
    enter_playing(&mut app);

    // Lose once.
    start_boss_fight(&mut app, 0);
    kill_player(&mut app);
    return_to_farm(&mut app);

    // Try again: the player must be healthy and the boss must be fresh.
    start_boss_fight(&mut app, 0);
    let health = player_health(&mut app);
    assert_eq!(health.current, health.max, "the retry must not auto-fail");

    let boss = first_of::<Boss>(&mut app);
    let boss = *app.world().get::<Boss>(boss).unwrap();
    assert_eq!(boss.health, boss.max_health);
    assert_eq!(boss.phase, 1);

    // And the retry can actually be won.
    defeat_current_boss(&mut app);
    assert_eq!(phase(&app), DayPhase::Result);
    assert!(app.world().resource::<BossProgress>().boss_a);
}

#[test]
fn several_defeats_then_a_victory_still_progresses() {
    let mut app = setup_app();
    enter_playing(&mut app);

    for attempt in 0..3 {
        start_boss_fight(&mut app, 0);
        kill_player(&mut app);
        return_to_farm(&mut app);
        assert!(
            !app.world().resource::<BossProgress>().boss_a,
            "attempt {attempt} must not have counted as a win"
        );
    }

    start_boss_fight(&mut app, 0);
    defeat_current_boss(&mut app);
    assert_eq!(phase(&app), DayPhase::Result);
    assert!(app.world().resource::<BossProgress>().boss_a);
    assert!(
        app.world()
            .resource::<CropUnlocks>()
            .is_unlocked(CropType::CropA)
    );
}

#[test]
fn player_status_clears_when_returning_from_a_fight() {
    let mut app = setup_app();
    enter_playing(&mut app);
    start_boss_fight(&mut app, 0);

    app.world_mut()
        .resource_mut::<PlayerStatus>()
        .apply_reversal(30.0);
    assert!(app.world().resource::<PlayerStatus>().is_reversed());

    kill_player(&mut app);
    return_to_farm(&mut app);

    assert!(
        !app.world().resource::<PlayerStatus>().is_reversed(),
        "transient boss effects must not survive into the farm"
    );
}
