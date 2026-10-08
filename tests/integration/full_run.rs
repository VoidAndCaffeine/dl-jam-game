//! Whole-run integration tests.
//!
//! These drive the real `GamePlugin` through complete day/night loops: farm,
//! fight, lose or win, return, craft, and finally beat the dual boss. They exist
//! to catch state that leaks between attempts (the player's health being the
//! original culprit) and to prove the loop can actually be finished.

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use bevy::transform::TransformPlugin;
use dl_jam::GamePlugin;
use dl_jam::components::boss::{Boss, Dying};
use dl_jam::components::gear::{GearSet, MaterialType, RECIPE_COUNT};
use dl_jam::components::player::{Health, Player};
use dl_jam::components::pot::{CropType, Pot, PotState};
use dl_jam::events::{CropHarvested, DamageDealt, InteractionEvent, InteractionType};
use dl_jam::levels::LevelId;
use dl_jam::plugins::interaction::BossArenaEntry;
use dl_jam::resources::boss_progress::BossProgress;
use dl_jam::resources::crafting_menu::CraftingMenu;
use dl_jam::resources::drop_rng::DropRng;
use dl_jam::resources::farm::CropUnlocks;
use dl_jam::resources::inventory::Inventory;
use dl_jam::resources::level::ActiveLevel;
use dl_jam::resources::player_attack_state::PlayerAttackState;
use dl_jam::resources::run_data::PlayerGear;
use dl_jam::states::{DayPhase, GameState};
use std::time::Duration;

/// The crops and materials the eight recipes consume in total.
const CROPS_PER_TYPE: u32 = 9;
const NEEDED_MATERIALS: [(MaterialType, u32); 4] = [
    (MaterialType::BossA1, 3),
    (MaterialType::BossA2, 6),
    (MaterialType::BossB1, 3),
    (MaterialType::BossB2, 6),
];

fn setup_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin, GamePlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            16,
        )));
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

fn first_of<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .iter(app.world())
        .next()
        .expect("expected at least one")
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
    step(app, 2);
    assert_eq!(phase(app), DayPhase::BossSelect);

    for _ in 0..target_index {
        tap(app, KeyCode::ArrowDown);
    }
    tap(app, KeyCode::Enter);
    tap(app, KeyCode::Enter);
    step(app, 3);
    assert_eq!(phase(app), DayPhase::BossFight);
}

/// Weakens the live boss and lands one light swing, exercising the real attack,
/// damage and defeat pipeline.
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

    // The lethal blow starts the death clip; skip to the end of it.
    if let Some(mut dying) = app.world_mut().get_mut::<Dying>(boss) {
        dying.remaining = 0.0;
    }
    step(app, 3);
}

/// Empties the shared dual-boss pool and skips both death clips.
fn defeat_dual_boss(app: &mut App) {
    let boss = first_of::<Boss>(app);
    app.world_mut().write_message(DamageDealt {
        target: boss,
        amount: 1_000_000.0,
        raw: 1_000_000.0,
    });
    step(app, 3);

    let halves: Vec<Entity> = app
        .world_mut()
        .query_filtered::<Entity, With<Dying>>()
        .iter(app.world())
        .collect();
    for entity in halves {
        if let Some(mut dying) = app.world_mut().get_mut::<Dying>(entity) {
            dying.remaining = 0.0;
        }
    }
    step(app, 3);
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
    step(app, 3);
    assert_eq!(phase(app), DayPhase::Farming);
    assert_eq!(active(app).id, LevelId::Farm);
}

/// Spends one in-game day on the farm: empty pots are planted with `crop`,
/// planted pots are watered, and ripe pots are harvested into the inventory.
fn farm_day(app: &mut App, crop: CropType) {
    let mut harvested: Vec<CropType> = Vec::new();
    {
        let mut query = app.world_mut().query::<&mut Pot>();
        for mut pot in query.iter_mut(app.world_mut()) {
            match pot.state {
                PotState::Empty => pot.plant(crop),
                PotState::Planted => {
                    pot.water();
                }
                PotState::Watered => {}
                PotState::Ready => {
                    if let Some(grown) = pot.harvest() {
                        harvested.push(grown);
                    }
                }
            }
        }
    }
    for grown in harvested {
        app.world_mut().write_message(CropHarvested(grown));
    }
    app.update();
}

/// Runs whole days until the inventory holds `target` of `crop`. Every day is
/// spent on the same boss, matching the game's one-day-one-attempt loop.
fn farm_until(app: &mut App, crop: CropType, target: u32, boss_index: usize, max_days: usize) {
    for _ in 0..max_days {
        if app.world().resource::<Inventory>().crop_count(crop) >= target {
            return;
        }
        farm_day(app, crop);
        start_boss_fight(app, boss_index);
        defeat_current_boss(app);
        return_to_farm(app);
    }
    panic!("{crop:?} did not reach {target} within {max_days} days");
}

fn has_all_crops(app: &App) -> bool {
    let crops = &app.world().resource::<Inventory>().crops;
    CropType::ALL
        .iter()
        .all(|crop| crops.get(crop).copied().unwrap_or(0) >= CROPS_PER_TYPE)
}

fn has_all_materials(app: &App) -> bool {
    let inventory = app.world().resource::<Inventory>();
    NEEDED_MATERIALS
        .iter()
        .all(|(material, amount)| inventory.material_count(*material) >= *amount)
}

/// Keeps fighting a boss outdoors until every Master-piece material has dropped.
fn grind_materials(app: &mut App, max_days: usize) {
    for _ in 0..max_days {
        if has_all_materials(app) {
            return;
        }
        let needs_a = {
            let inventory = app.world().resource::<Inventory>();
            inventory.material_count(MaterialType::BossA1) < 3
                || inventory.material_count(MaterialType::BossA2) < 6
        };
        farm_day(app, CropType::Starter);
        start_boss_fight(app, if needs_a { 0 } else { 1 });
        defeat_current_boss(app);
        return_to_farm(app);
    }
    panic!("materials did not fill within {max_days} days");
}

/// Crafts every recipe row through the real crafting menu.
fn craft_all(app: &mut App) {
    app.world_mut().resource_mut::<CraftingMenu>().open_menu();
    for row in 0..RECIPE_COUNT {
        app.world_mut().resource_mut::<CraftingMenu>().selected = row;
        tap(app, KeyCode::Enter);
    }
    app.world_mut().resource_mut::<CraftingMenu>().close_menu();
}

fn assert_victory(app: &mut App) {
    assert_eq!(phase(app), DayPhase::Result);
    tap(app, KeyCode::Space);
    step(app, 3);
    assert_eq!(
        app.world().resource::<State<GameState>>().get(),
        &GameState::Victory
    );
}

#[test]
fn full_run_victory_without_a_single_defeat() {
    let mut app = setup_app();
    enter_playing(&mut app);

    start_boss_fight(&mut app, 0);
    defeat_current_boss(&mut app);
    return_to_farm(&mut app);
    assert!(app.world().resource::<BossProgress>().boss_a);

    start_boss_fight(&mut app, 1);
    defeat_current_boss(&mut app);
    return_to_farm(&mut app);
    assert!(app.world().resource::<BossProgress>().boss_b);
    assert!(app.world().resource::<BossProgress>().dual_boss_unlocked);

    start_boss_fight(&mut app, 2);
    defeat_dual_boss(&mut app);
    assert_victory(&mut app);
}

#[test]
fn full_run_with_a_defeat_on_every_boss() {
    let mut app = setup_app();
    enter_playing(&mut app);

    // Boss A: lose, then win.
    start_boss_fight(&mut app, 0);
    kill_player(&mut app);
    return_to_farm(&mut app);
    assert!(!app.world().resource::<BossProgress>().boss_a);
    start_boss_fight(&mut app, 0);
    defeat_current_boss(&mut app);
    return_to_farm(&mut app);
    assert!(app.world().resource::<BossProgress>().boss_a);

    // Boss B: lose, then win.
    start_boss_fight(&mut app, 1);
    kill_player(&mut app);
    return_to_farm(&mut app);
    assert!(!app.world().resource::<BossProgress>().boss_b);
    start_boss_fight(&mut app, 1);
    defeat_current_boss(&mut app);
    return_to_farm(&mut app);
    assert!(app.world().resource::<BossProgress>().boss_b);
    assert!(app.world().resource::<BossProgress>().dual_boss_unlocked);

    // Dual: lose, then win the whole run.
    start_boss_fight(&mut app, 2);
    kill_player(&mut app);
    return_to_farm(&mut app);
    assert!(!app.world().resource::<BossProgress>().dual_boss_beaten);
    assert_eq!(
        player_health(&mut app).current,
        player_health(&mut app).max,
        "the dual retry must start fresh"
    );

    start_boss_fight(&mut app, 2);
    defeat_dual_boss(&mut app);
    assert_victory(&mut app);
}

#[test]
fn full_run_grows_every_crop_crafts_every_piece_and_wins() {
    let mut app = setup_app();
    enter_playing(&mut app);
    // Pin the boss drops so the run replays identically.
    app.world_mut().insert_resource(DropRng::seeded(2024));

    // Beat both single bosses to unlock every crop and the dual fight.
    start_boss_fight(&mut app, 0);
    defeat_current_boss(&mut app);
    return_to_farm(&mut app);
    start_boss_fight(&mut app, 1);
    defeat_current_boss(&mut app);
    return_to_farm(&mut app);

    let unlocks = app.world().resource::<CropUnlocks>();
    assert!(unlocks.is_unlocked(CropType::CropA));
    assert!(unlocks.is_unlocked(CropType::CropB));
    assert!(app.world().resource::<BossProgress>().dual_boss_unlocked);

    // Grow a full harvest of each crop. Days only pass on the back of a boss
    // attempt, which is exactly the game's day/night loop.
    farm_until(&mut app, CropType::Starter, CROPS_PER_TYPE, 0, 12);
    farm_until(&mut app, CropType::CropA, CROPS_PER_TYPE, 0, 12);
    farm_until(&mut app, CropType::CropB, CROPS_PER_TYPE, 1, 14);

    // Top up the rarer drops the two Master pieces need.
    grind_materials(&mut app, 20);

    assert!(
        has_all_crops(&app),
        "the farm should have produced all crops"
    );
    assert!(
        has_all_materials(&app),
        "the bosses should have dropped all materials"
    );

    craft_all(&mut app);
    let gear = app.world().resource::<PlayerGear>();
    for set in GearSet::ALL {
        assert!(gear.owns_set(set), "missing a piece of {set:?}");
    }
    assert_eq!(gear.owned.len(), RECIPE_COUNT);

    // Dual boss: lose once, then win the run.
    start_boss_fight(&mut app, 2);
    kill_player(&mut app);
    return_to_farm(&mut app);
    start_boss_fight(&mut app, 2);
    defeat_dual_boss(&mut app);
    assert_victory(&mut app);
}
