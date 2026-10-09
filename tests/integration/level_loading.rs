use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use bevy::transform::TransformPlugin;
use dl_jam::GamePlugin;
use dl_jam::components::collider::Collider;
use dl_jam::components::player::{Movement, Player};
use dl_jam::components::pot::Pot;
use dl_jam::levels::{LevelId, PropKind};
use dl_jam::plugins::gear::CRAFTING_STATION_SIZE;
use dl_jam::plugins::interaction::{
    BossArenaEntry, CraftingStation, FarmPot, HIGHLIGHT_Z, HighlightMarker,
};
use dl_jam::plugins::level::FLOOR_Z;
use dl_jam::resources::level::{
    ActiveLevel, LevelEntity, LevelRequest, PlayerSpawn, prop_position,
};
use dl_jam::states::GameState;

fn setup_app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin, GamePlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        // The default strategy reads the wall clock, which does not move during a
        // test, so drive the timestep by hand instead.
        .insert_resource(TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(16),
        ));
    app
}

fn enter_playing(app: &mut App) {
    // Boot settles into the title screen, then a new run begins.
    for _ in 0..3 {
        app.update();
    }
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    for _ in 0..2 {
        app.update();
    }
}

fn step(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

fn switch_to(app: &mut App, id: LevelId) {
    app.world_mut().resource_mut::<LevelRequest>().0 = Some(id);
    app.update();
}

fn hold(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
}

fn count<T: Component>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .iter(app.world())
        .count()
}

fn active(app: &App) -> ActiveLevel {
    app.world().resource::<ActiveLevel>().clone()
}

fn grid(app: &App) -> dl_jam::levels::SolidGrid {
    app.world().resource::<dl_jam::levels::SolidGrid>().clone()
}

fn player_position(app: &mut App) -> Vec2 {
    app.world_mut()
        .query_filtered::<&Transform, With<Player>>()
        .single(app.world())
        .expect("a player")
        .translation
        .truncate()
}

fn first_of<T: Component>(app: &mut App) -> Entity {
    let world = app.world_mut();
    world
        .query_filtered::<Entity, With<T>>()
        .iter(world)
        .next()
        .expect("expected at least one")
}

fn highlight_of(world: &mut World, parent: Entity) -> Option<Entity> {
    let children = world.get::<Children>(parent)?;
    children
        .iter()
        .find(|child| world.get::<HighlightMarker>(*child).is_some())
}

fn position_of<T: Component>(app: &mut App) -> Vec2 {
    let world = app.world_mut();
    let mut query = world.query_filtered::<&Transform, With<T>>();
    query
        .iter(world)
        .map(|transform| transform.translation.truncate())
        .next()
        .expect("expected at least one")
}

#[test]
fn entering_play_loads_the_farm_and_places_everything() {
    let mut app = setup_app();
    enter_playing(&mut app);

    let level = active(&app);
    assert_eq!(level.id, LevelId::Farm);
    assert_eq!((level.def.width, level.def.height), (40, 24));

    assert_eq!(count::<Pot>(&mut app), 9);
    assert_eq!(count::<CraftingStation>(&mut app), 1);
    assert_eq!(count::<BossArenaEntry>(&mut app), 1);
    assert_eq!(count::<FarmPot>(&mut app), 9);

    let grid = grid(&app);
    assert_eq!(grid.width(), 40);
    assert_eq!(grid.height(), 24);
}

#[test]
fn the_pots_sit_where_the_level_file_puts_them() {
    let mut app = setup_app();
    enter_playing(&mut app);

    let level = active(&app);
    let grid = grid(&app);
    let mut positions: Vec<Vec2> = app
        .world_mut()
        .query_filtered::<&Transform, With<Pot>>()
        .iter(app.world())
        .map(|transform| transform.translation.truncate())
        .collect();
    positions.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap()
            .then(a.y.partial_cmp(&b.y).unwrap())
    });

    let mut expected: Vec<Vec2> = level
        .def
        .props_of(PropKind::Pot)
        .into_iter()
        .map(|(col, row)| grid.prop_center(&level.def, col, row))
        .collect();
    expected.sort_by(|a, b| {
        a.x.partial_cmp(&b.x)
            .unwrap()
            .then(a.y.partial_cmp(&b.y).unwrap())
    });

    assert_eq!(positions, expected);
}

#[test]
fn pot_indices_follow_the_level_file_order() {
    let mut app = setup_app();
    enter_playing(&mut app);

    let indices: Vec<(usize, Vec2)> = app
        .world_mut()
        .query::<(&Pot, &Transform)>()
        .iter(app.world())
        .map(|(pot, transform)| (pot.index, transform.translation.truncate()))
        .collect();

    let level = active(&app);
    let grid = grid(&app);
    for (index, (col, row)) in level.def.props_of(PropKind::Pot).iter().enumerate() {
        let (_, position) = indices
            .iter()
            .find(|(pot_index, _)| *pot_index == index)
            .unwrap_or_else(|| panic!("pot {index} is missing"));
        assert_eq!(
            *position,
            grid.prop_center(&level.def, *col, *row),
            "pot {index}"
        );
    }
}

#[test]
fn the_station_and_gate_come_from_their_markers() {
    let mut app = setup_app();
    enter_playing(&mut app);

    let level = active(&app);
    let grid = grid(&app);

    let station = position_of::<CraftingStation>(&mut app);
    let gate = position_of::<BossArenaEntry>(&mut app);

    assert_eq!(
        station,
        prop_position(&grid, &level.def, PropKind::CraftingStation).unwrap()
    );
    assert_eq!(
        gate,
        prop_position(&grid, &level.def, PropKind::ArenaGate).unwrap()
    );
}

#[test]
fn the_player_spawns_on_the_level_marker_not_the_origin() {
    let mut app = setup_app();
    enter_playing(&mut app);

    let expected = app.world().resource::<PlayerSpawn>().position;
    assert_ne!(expected, Vec2::ZERO);
    assert_eq!(player_position(&mut app), expected);
}

#[test]
fn the_player_spawns_clear_of_every_wall() {
    let mut app = setup_app();
    enter_playing(&mut app);

    let grid = grid(&app);
    let spawn = player_position(&mut app);
    assert!(!grid.aabb_hits_solid(spawn, Vec2::splat(16.0)));
}

#[test]
fn switching_rooms_replaces_the_props_and_the_grid() {
    let mut app = setup_app();
    enter_playing(&mut app);
    assert_eq!(count::<Pot>(&mut app), 9);

    switch_to(&mut app, LevelId::ArenaA);

    let level = active(&app);
    assert_eq!(level.id, LevelId::ArenaA);
    assert_eq!((level.def.width, level.def.height), (40, 23));
    assert_eq!(count::<Pot>(&mut app), 0, "arenas have no farming");
    assert_eq!(count::<CraftingStation>(&mut app), 0);
    assert_eq!(count::<BossArenaEntry>(&mut app), 0);
    assert_eq!(grid(&app).width(), 40);
}

#[test]
fn switching_rooms_moves_the_player_to_the_new_spawn() {
    let mut app = setup_app();
    enter_playing(&mut app);
    let farm_spawn = player_position(&mut app);

    switch_to(&mut app, LevelId::ArenaB);
    let arena_spawn = player_position(&mut app);

    assert_ne!(farm_spawn, arena_spawn);
    assert!(!grid(&app).aabb_hits_solid(arena_spawn, Vec2::splat(16.0)));
}

#[test]
fn going_back_to_the_farm_rebuilds_its_props() {
    let mut app = setup_app();
    enter_playing(&mut app);
    switch_to(&mut app, LevelId::ArenaDual);

    assert_eq!(count::<Pot>(&mut app), 0);
    assert_eq!((grid(&app).width(), grid(&app).height()), (36, 19));

    switch_to(&mut app, LevelId::Farm);
    assert_eq!(count::<Pot>(&mut app), 9);
    assert_eq!(count::<CraftingStation>(&mut app), 1);
}

#[test]
fn requesting_the_level_that_is_already_loaded_does_nothing() {
    let mut app = setup_app();
    enter_playing(&mut app);

    let before = count::<Pot>(&mut app);
    let spawn = app.world().resource::<PlayerSpawn>().position;
    switch_to(&mut app, LevelId::Farm);
    assert_eq!(count::<Pot>(&mut app), before);
    assert_eq!(app.world().resource::<PlayerSpawn>().position, spawn);
}

#[test]
fn every_level_loads_and_clears_cleanly() {
    for id in LevelId::ALL {
        let mut app = setup_app();
        enter_playing(&mut app);

        switch_to(&mut app, id);
        assert_eq!(active(&app).id, id);
        assert!(!grid(&app).aabb_hits_solid(player_position(&mut app), Vec2::splat(16.0)));

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();

        assert_eq!(
            count::<LevelEntity>(&mut app),
            0,
            "{} left entities",
            id.file_name()
        );
        assert_eq!(count::<Pot>(&mut app), 0);
        assert_eq!(count::<Player>(&mut app), 0);
    }
}

#[test]
fn only_the_active_rooms_entities_are_tagged() {
    let mut app = setup_app();
    enter_playing(&mut app);
    switch_to(&mut app, LevelId::ArenaA);

    let mut levels: Vec<LevelId> = app
        .world_mut()
        .query::<&LevelEntity>()
        .iter(app.world())
        .map(|entity| entity.level)
        .collect();
    levels.sort_by_key(|id| id.file_name());

    assert_eq!(levels.len(), 1, "one root marker for the active room");
    assert!(levels.contains(&LevelId::ArenaA));
}

#[test]
fn the_arena_gate_is_only_in_the_farm() {
    let mut app = setup_app();
    enter_playing(&mut app);
    assert_eq!(count::<BossArenaEntry>(&mut app), 1);

    for arena in [LevelId::ArenaA, LevelId::ArenaB, LevelId::ArenaDual] {
        switch_to(&mut app, arena);
        assert_eq!(
            count::<BossArenaEntry>(&mut app),
            0,
            "{}",
            arena.file_name()
        );
    }
}

#[test]
fn the_station_is_a_solid_obstacle_the_player_cannot_walk_through() {
    let mut app = setup_app();
    enter_playing(&mut app);

    let grid = grid(&app);
    let station = app
        .world_mut()
        .query_filtered::<(&Transform, &Collider), With<CraftingStation>>()
        .single(app.world())
        .map(|(transform, collider)| (transform.translation.truncate(), collider.size * 0.5))
        .unwrap();

    let from = station.0 + Vec2::new(-CRAFTING_STATION_SIZE, 0.0);
    let obstacles = [station];
    let walked =
        grid.move_and_collide_with(&obstacles, Vec2::splat(16.0), from, Vec2::new(400.0, 0.0));

    assert!(
        walked.x < station.0.x,
        "should stop before the station, got {walked:?}"
    );
}

#[test]
fn the_player_cannot_walk_out_through_the_farm_wall() {
    let mut app = setup_app();
    enter_playing(&mut app);

    let grid = grid(&app);
    let spawn = player_position(&mut app);
    let half = Vec2::splat(16.0);
    let mut walked = spawn;
    for _ in 0..60 {
        walked = grid.move_and_collide(half, walked, Vec2::new(60.0, 0.0));
    }

    assert!(
        walked.x < grid.world_size().x * 0.5,
        "stopped inside the room"
    );
    assert!(!grid.aabb_hits_solid(walked, half));
}

#[test]
fn movement_is_driven_by_the_movement_component() {
    let mut app = setup_app();
    enter_playing(&mut app);

    {
        let world = app.world_mut();
        let mut query = world.query::<&mut Movement>();
        let mut movement = query.single_mut(world).expect("a player movement");
        movement.input_direction = Vec2::X;
        movement.speed = 200.0;
    }
    app.update();

    let grid = grid(&app);
    let position = player_position(&mut app);
    assert!(
        !grid.aabb_hits_solid(position, Vec2::splat(16.0)),
        "still inside the room after moving right"
    );
}

#[test]
fn holding_a_key_walks_the_player_through_the_fixed_timestep() {
    let mut app = setup_app();
    enter_playing(&mut app);
    let start = player_position(&mut app);

    hold(&mut app, KeyCode::KeyD);
    step(&mut app, 120);

    let moved = player_position(&mut app);
    assert!(
        moved.x > start.x + 50.0,
        "player should walk right: {start:?} -> {moved:?}"
    );
}

#[test]
fn interactables_draw_their_outline_through_a_material_not_a_child() {
    let mut app = setup_app();
    enter_playing(&mut app);

    // The pots, the crafting station and the boss door all highlight through the
    // shared outline material, so none of them spawn a child highlight sprite.
    let pot = first_of::<Pot>(&mut app);
    assert!(
        highlight_of(app.world_mut(), pot).is_none(),
        "the mound's shader outline replaces the child highlight sprite"
    );

    let station = first_of::<CraftingStation>(&mut app);
    assert!(
        highlight_of(app.world_mut(), station).is_none(),
        "the station's shader outline replaces the child highlight sprite"
    );

    let door = first_of::<BossArenaEntry>(&mut app);
    assert!(
        highlight_of(app.world_mut(), door).is_none(),
        "the door's shader outline replaces the child highlight sprite"
    );
}

#[test]
fn the_floor_is_drawn_behind_the_highlights() {
    // The tiles are opaque and drawn as one quad. If the floor sat in front of
    // the highlight layer the outlines would be invisible, so guard the ordering
    // the rest of the project depends on.
    const {
        assert!(
            FLOOR_Z < HIGHLIGHT_Z,
            "the floor must sit behind the highlight layer"
        )
    };
}
