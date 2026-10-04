use bevy::prelude::*;
use bevy::ecs::message::MessageWriter;
use bevy::state::state::State;
use crate::components::player::{INTERACTION_RANGE, Player};
use crate::events::InteractionEvent;
use crate::states::{DayPhase, GameState};
use crate::utils::interaction_math::{find_closest_in_range, find_closest_to_ray, resolve_interaction_type_from_queries};

#[derive(Component, Reflect, Default, Debug)]
pub struct Interactable;

impl Interactable {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Component, Reflect, Default, Debug)]
pub struct FarmPot;

#[derive(Component, Reflect, Default, Debug)]
pub struct CraftingStation;

#[derive(Component, Reflect, Default, Debug)]
pub struct BossArenaEntry;

#[derive(Component, Reflect, Default, Debug)]
pub struct NPC;

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<InteractionEvent>()
            .add_systems(Update, player_proximity_interaction)
            .add_systems(Update, mouse_raycast_interaction)
            .add_systems(Update, highlight_interactables_in_range);
    }
}

fn player_proximity_interaction(
    mut events: MessageWriter<InteractionEvent>,
    player_query: Query<&GlobalTransform, With<Player>>,
    interactables: Query<(Entity, &GlobalTransform), With<Interactable>>,
    farm_pots: Query<&FarmPot>,
    crafting_stations: Query<&CraftingStation>,
    boss_arenas: Query<&BossArenaEntry>,
    npcs: Query<&NPC>,
    keys: Res<ButtonInput<KeyCode>>,
    game_state: Res<State<GameState>>,
    day_phase: Res<State<DayPhase>>,
) {
    if !matches!(game_state.get(), GameState::Playing) || !matches!(day_phase.get(), DayPhase::Farming) {
        return;
    }
    if !keys.just_pressed(KeyCode::Space) {
        return;
    }

    let Ok(player_transform) = player_query.single() else { return };
    let player_pos = player_transform.translation().truncate();

    let interactable_positions: Vec<(Entity, Vec2)> = interactables
        .iter()
        .map(|(entity, transform)| (entity, transform.translation().truncate()))
        .collect();

    eprintln!("[DEBUG] Space pressed! Player at {:?}, {} interactables found", player_pos, interactable_positions.len());
    for (entity, pos) in &interactable_positions {
        let dist = player_pos.distance(*pos);
        eprintln!("[DEBUG]   Entity {:?} at {:?}, distance: {:.2}, in range: {}", entity, pos, dist, dist <= INTERACTION_RANGE);
    }

    let closest_entity = find_closest_in_range(player_pos, &interactable_positions, INTERACTION_RANGE);

    if let Some(entity) = closest_entity {
        eprintln!("[DEBUG] Closest in range: {:?}", entity);
        let interaction_type = resolve_interaction_type_from_queries(
            entity,
            &farm_pots,
            &crafting_stations,
            &boss_arenas,
            &npcs,
        );
        events.write(InteractionEvent {
            entity,
            interaction_type,
        });
        eprintln!("[DEBUG] Interaction event sent for {:?}", entity);
    } else {
        eprintln!("[DEBUG] No interactable in range");
    }
}

fn mouse_raycast_interaction(
    mut events: MessageWriter<InteractionEvent>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    interactables: Query<(Entity, &GlobalTransform), With<Interactable>>,
    farm_pots: Query<&FarmPot>,
    crafting_stations: Query<&CraftingStation>,
    boss_arenas: Query<&BossArenaEntry>,
    npcs: Query<&NPC>,
    player_query: Query<&GlobalTransform, With<Player>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    game_state: Res<State<GameState>>,
    day_phase: Res<State<DayPhase>>,
) {
    if !matches!(game_state.get(), GameState::Playing) || !matches!(day_phase.get(), DayPhase::Farming) {
        return;
    }
    if !mouse_input.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = windows.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };

    let Ok((camera, camera_transform)) = cameras.single() else { return };
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) else { return };

    let Ok(player_transform) = player_query.single() else { return };
    let player_pos = player_transform.translation().truncate();

    let interactable_positions: Vec<(Entity, Vec2)> = interactables
        .iter()
        .map(|(entity, transform)| (entity, transform.translation().truncate()))
        .collect();

    eprintln!("[DEBUG] Left click! Player at {:?}, cursor at {:?}, ray origin: {:?}, {} interactables found", player_pos, cursor_pos, ray.origin.truncate(), interactable_positions.len());
    for (entity, pos) in &interactable_positions {
        let dist = player_pos.distance(*pos);
        eprintln!("[DEBUG]   Entity {:?} at {:?}, distance: {:.2}, in range: {}", entity, pos, dist, dist <= INTERACTION_RANGE);
    }

    let in_range: Vec<(Entity, Vec2)> = interactable_positions
        .into_iter()
        .filter(|(_, pos)| player_pos.distance(*pos) <= INTERACTION_RANGE)
        .collect();

    eprintln!("[DEBUG] {} interactables in player range", in_range.len());

    let closest_entity = find_closest_to_ray(ray.origin.truncate(), &in_range);

    if let Some(entity) = closest_entity {
        eprintln!("[DEBUG] Closest to ray in range: {:?}", entity);
        let interaction_type = resolve_interaction_type_from_queries(
            entity,
            &farm_pots,
            &crafting_stations,
            &boss_arenas,
            &npcs,
        );
        events.write(InteractionEvent {
            entity,
            interaction_type,
        });
        eprintln!("[DEBUG] Interaction event sent for {:?}", entity);
    } else {
        eprintln!("[DEBUG] No interactable in range of player AND ray");
    }
}
        
fn highlight_interactables_in_range(
    player_query: Query<&Transform, With<Player>>,
    interactables: Query<(Entity, &Transform), With<Interactable>>,
    mut commands: Commands,
) {
    let Ok(player_transform) = player_query.single() else { return };
    let player_pos = player_transform.translation.truncate();

    for (entity, transform) in interactables.iter() {
        let distance = player_pos.distance(transform.translation.truncate());
        let mut entity_commands = commands.entity(entity);

        if distance <= INTERACTION_RANGE {
            entity_commands.try_insert(Outline {
                width: Val::Px(2.0),
                offset: Val::Px(2.0),
                color: Color::srgb(1.0, 1.0, 0.0),
            });
        } else {
            entity_commands.remove::<Outline>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;
    use bevy::transform::TransformPlugin;
    use bevy::state::app::StatesPlugin;
    use bevy::ecs::message::MessageReader;
    use crate::events::InteractionType;

    #[derive(Resource, Default)]
    struct CapturedEvents(Vec<InteractionEvent>);

    fn capture_events(mut captured: ResMut<CapturedEvents>, mut reader: MessageReader<InteractionEvent>) {
        for event in reader.read() {
            captured.0.push(event.clone());
        }
    }

    fn setup_interaction_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<CapturedEvents>()
            .add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_message::<InteractionEvent>()
            .add_systems(Update, player_proximity_interaction)
            .add_systems(Update, mouse_raycast_interaction)
            .add_systems(Update, highlight_interactables_in_range)
            .add_systems(Update, capture_events.after(player_proximity_interaction).after(mouse_raycast_interaction));

        app.world_mut().spawn((
            Player,
            Transform::from_xyz(0.0, 0.0, 1.0),
        ));

        app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::Playing);
        app.world_mut().resource_mut::<NextState<DayPhase>>().set(DayPhase::Farming);
        
        // Process state transitions
        app.update();
        app
    }

    fn spawn_interactable(app: &mut App, pos: Vec2, marker: impl Component) -> Entity {
        app.world_mut().spawn((
            Interactable,
            marker,
            Transform::from_xyz(pos.x, pos.y, 0.0),
        )).id()
    }

    fn get_captured_events(app: &mut App) -> Vec<InteractionEvent> {
        let captured = app.world().resource::<CapturedEvents>();
        captured.0.clone()
    }

    fn clear_captured_events(app: &mut App) {
        let mut captured = app.world_mut().resource_mut::<CapturedEvents>();
        captured.0.clear();
    }

    #[test]
    fn space_interacts_with_closest_in_range() {
        let mut app = setup_interaction_app();
        let e1 = spawn_interactable(&mut app, Vec2::new(20.0, 0.0), FarmPot);
        let e2 = spawn_interactable(&mut app, Vec2::new(10.0, 0.0), FarmPot);
        app.update(); // Allow TransformPropagation to compute GlobalTransform

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].entity, e2);
        assert_eq!(events[0].interaction_type, InteractionType::FarmAction);
    }

    #[test]
    fn space_no_interaction_when_none_in_range() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(100.0, 0.0), FarmPot);
        app.update(); // Allow TransformPropagation to compute GlobalTransform

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn space_no_interaction_when_no_pots() {
        let mut app = setup_interaction_app();

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn space_only_in_farming_phase() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(10.0, 0.0), FarmPot);

        app.world_mut().resource_mut::<NextState<DayPhase>>().set(DayPhase::BossSelect);
        app.update();

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn space_only_in_playing_state() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(10.0, 0.0), FarmPot);

        app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::LoadingAssets);
        app.update();

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn space_emits_correct_interaction_type() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(10.0, 0.0), FarmPot);
        spawn_interactable(&mut app, Vec2::new(15.0, 0.0), CraftingStation);
        app.update(); // Allow TransformPropagation to compute GlobalTransform

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].interaction_type, InteractionType::FarmAction);
    }

    #[test]
    fn highlight_adds_outline_in_range() {
        let mut app = setup_interaction_app();
        let entity = spawn_interactable(&mut app, Vec2::new(20.0, 0.0), FarmPot);

        app.update();

        let has_outline = app.world().get::<Outline>(entity).is_some();
        assert!(has_outline);
    }

    #[test]
    fn highlight_removes_outline_out_of_range() {
        let mut app = setup_interaction_app();
        let entity = spawn_interactable(&mut app, Vec2::new(100.0, 0.0), FarmPot);

        app.update();

        let has_outline = app.world().get::<Outline>(entity).is_some();
        assert!(!has_outline);
    }

    #[test]
    fn highlight_updates_when_player_moves() {
        let mut app = setup_interaction_app();
        let entity = spawn_interactable(&mut app, Vec2::new(0.0, 0.0), FarmPot);

        app.update();
        assert!(app.world().get::<Outline>(entity).is_some());

        let mut player_transform = app.world_mut().query_filtered::<&mut Transform, With<Player>>().single(app.world_mut()).unwrap().clone();
        player_transform.translation.x = 100.0;
        app.world_mut().query_filtered::<&mut Transform, With<Player>>().single_mut(app.world_mut()).unwrap().translation = player_transform.translation;
        app.update();

        assert!(!app.world().get::<Outline>(entity).is_some());
    }

    #[test]
    fn highlight_only_on_interactables() {
        let mut app = setup_interaction_app();
        let entity = app.world_mut().spawn((
            Transform::from_xyz(10.0, 0.0, 0.0),
            GlobalTransform::default(),
        )).id();

        app.update();

        let has_outline = app.world().get::<Outline>(entity).is_some();
        assert!(!has_outline);
    }

    #[test]
    fn interactable_creation() {
        let interactable = Interactable::new();
        let _ = interactable;
    }

    #[test]
    fn interactable_default() {
        let interactable = Interactable::default();
        let _ = interactable;
    }
}