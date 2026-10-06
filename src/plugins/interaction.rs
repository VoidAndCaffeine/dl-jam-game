use crate::components::player::{INTERACTION_RANGE, Player};
use crate::events::InteractionEvent;
use crate::resources::crafting_menu::CraftingMenu;
use crate::resources::crop_select::CropSelectMenu;
use crate::resources::inventory_panel::InventoryPanel;
use crate::states::Phase;
use crate::utils::interaction_math::{
    find_closest_in_range, find_closest_to_ray, resolve_interaction_type_from_queries,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

#[derive(Component, Reflect, Default, Debug)]
pub struct Interactable;

impl Interactable {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Component, Reflect, Default, Debug)]
pub struct HighlightMarker;

/// Highlights sit just behind their parent sprite. The room floor must stay
/// further back than this or it covers them.
pub const HIGHLIGHT_Z: f32 = -0.1;

#[derive(Component, Reflect, Default, Debug)]
pub struct FarmPot;

#[derive(Component, Reflect, Default, Debug)]
pub struct CraftingStation;

#[derive(Component, Reflect, Default, Debug)]
pub struct BossArenaEntry;

#[derive(Component, Reflect, Default, Debug)]
pub struct NPC;

/// Everything both interaction systems need to find and classify interactables.
#[derive(bevy::ecs::system::SystemParam)]
pub struct InteractableLookups<'w, 's> {
    pub player: Query<'w, 's, &'static GlobalTransform, With<Player>>,
    pub interactables: Query<'w, 's, (Entity, &'static GlobalTransform), With<Interactable>>,
    pub farm_pots: Query<'w, 's, &'static FarmPot>,
    pub pots: Query<'w, 's, &'static crate::components::pot::Pot>,
    pub crafting_stations: Query<'w, 's, &'static CraftingStation>,
    pub boss_arenas: Query<'w, 's, &'static BossArenaEntry>,
    pub npcs: Query<'w, 's, &'static NPC>,
}

/// The panels that freeze world interaction while they are open.
#[derive(bevy::ecs::system::SystemParam)]
pub struct OpenPanels<'w> {
    menu: Res<'w, CraftingMenu>,
    inventory: Res<'w, InventoryPanel>,
    crop_select: Res<'w, CropSelectMenu>,
}

impl OpenPanels<'_> {
    fn any_open(&self) -> bool {
        self.menu.open || self.inventory.open || self.crop_select.open
    }
}

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryPanel>()
            .init_resource::<CropSelectMenu>()
            .add_message::<InteractionEvent>()
            .add_systems(Update, player_proximity_interaction)
            .add_systems(Update, mouse_raycast_interaction)
            .add_systems(Update, highlight_interactables_in_range);
    }
}

fn player_proximity_interaction(
    mut events: MessageWriter<InteractionEvent>,
    lookups: InteractableLookups,
    keys: Res<ButtonInput<KeyCode>>,
    panels: OpenPanels,
    phase: Phase,
) {
    if !phase.is_farming() || panels.any_open() {
        return;
    }
    if !keys.just_pressed(KeyCode::Space) {
        return;
    }

    let Ok(player_transform) = lookups.player.single() else {
        return;
    };
    let player_pos = player_transform.translation().truncate();

    let interactable_positions: Vec<(Entity, Vec2)> = lookups
        .interactables
        .iter()
        .filter(|(entity, _)| {
            // Skip watered pots - they are not interactable until next day
            lookups
                .pots
                .get(*entity)
                .map(|p| p.state != crate::components::pot::PotState::Watered)
                .unwrap_or(true)
        })
        .map(|(entity, transform)| (entity, transform.translation().truncate()))
        .collect();

    log::debug!(
        "space pressed at {player_pos:?} with {} interactables",
        interactable_positions.len()
    );

    let closest_entity =
        find_closest_in_range(player_pos, &interactable_positions, INTERACTION_RANGE);

    if let Some(entity) = closest_entity {
        let interaction_type = resolve_interaction_type_from_queries(
            entity,
            &lookups.farm_pots,
            &lookups.crafting_stations,
            &lookups.boss_arenas,
            &lookups.npcs,
        );
        events.write(InteractionEvent {
            entity,
            interaction_type,
        });
    }
}

fn mouse_raycast_interaction(
    mut events: MessageWriter<InteractionEvent>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    lookups: InteractableLookups,
    mouse_input: Res<ButtonInput<MouseButton>>,
    panels: OpenPanels,
    phase: Phase,
) {
    if !phase.is_farming() || panels.any_open() {
        return;
    }
    if !mouse_input.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = windows.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) else {
        return;
    };

    let Ok(player_transform) = lookups.player.single() else {
        return;
    };
    let player_pos = player_transform.translation().truncate();

    let interactable_positions: Vec<(Entity, Vec2)> = lookups
        .interactables
        .iter()
        .filter(|(entity, _)| {
            // Skip watered pots - they are not interactable until next day
            lookups
                .pots
                .get(*entity)
                .map(|p| p.state != crate::components::pot::PotState::Watered)
                .unwrap_or(true)
        })
        .map(|(entity, transform)| (entity, transform.translation().truncate()))
        .collect();

    log::debug!(
        "left click at {cursor_pos:?}, ray from {:?}, {} interactables",
        ray.origin.truncate(),
        interactable_positions.len()
    );

    let in_range: Vec<(Entity, Vec2)> = interactable_positions
        .into_iter()
        .filter(|(_, pos)| player_pos.distance(*pos) <= INTERACTION_RANGE)
        .collect();

    let closest_entity = find_closest_to_ray(ray.origin.truncate(), &in_range);

    if let Some(entity) = closest_entity {
        let interaction_type = resolve_interaction_type_from_queries(
            entity,
            &lookups.farm_pots,
            &lookups.crafting_stations,
            &lookups.boss_arenas,
            &lookups.npcs,
        );
        events.write(InteractionEvent {
            entity,
            interaction_type,
        });
    }
}

fn highlight_interactables_in_range(
    player_query: Query<&Transform, With<Player>>,
    interactables: Query<(Entity, &Transform, &Children), With<Interactable>>,
    pots: Query<&crate::components::pot::Pot>,
    highlights: Query<&HighlightMarker>,
    mut visibility: Query<&mut Visibility>,
    panels: OpenPanels,
    phase: Phase,
) {
    if !phase.is_farming() {
        return;
    }
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();

    for (entity, transform, children) in interactables.iter() {
        let distance = player_pos.distance(transform.translation.truncate());

        let hidden = panels.any_open()
            || pots
                .get(entity)
                .map(|p| p.state == crate::components::pot::PotState::Watered)
                .unwrap_or(false);
        if hidden {
            // Still need to hide the highlight if it was previously visible
            for child in children.iter() {
                if highlights.get(child).is_ok() {
                    if let Ok(mut vis) = visibility.get_mut(child) {
                        *vis = Visibility::Hidden;
                    }
                    break;
                }
            }
            continue;
        }

        // Find the highlight child
        for child in children.iter() {
            if highlights.get(child).is_ok() {
                if let Ok(mut vis) = visibility.get_mut(child) {
                    if distance <= INTERACTION_RANGE {
                        *vis = Visibility::Visible;
                    } else {
                        *vis = Visibility::Hidden;
                    }
                }
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::InteractionType;
    use crate::states::{DayPhase, GameState};
    use bevy::ecs::message::MessageReader;
    use bevy::state::app::StatesPlugin;
    use bevy::transform::TransformPlugin;

    #[derive(Resource, Default)]
    struct CapturedEvents(Vec<InteractionEvent>);

    fn capture_events(
        mut captured: ResMut<CapturedEvents>,
        mut reader: MessageReader<InteractionEvent>,
    ) {
        for event in reader.read() {
            captured.0.push(event.clone());
        }
    }

    fn setup_interaction_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<CraftingMenu>()
            .init_resource::<InventoryPanel>()
            .init_resource::<CropSelectMenu>()
            .init_resource::<CapturedEvents>()
            .add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_message::<InteractionEvent>()
            .add_systems(Update, player_proximity_interaction)
            .add_systems(Update, mouse_raycast_interaction)
            .add_systems(Update, highlight_interactables_in_range)
            .add_systems(
                Update,
                capture_events
                    .after(player_proximity_interaction)
                    .after(mouse_raycast_interaction),
            );

        app.world_mut()
            .spawn((Player, Transform::from_xyz(0.0, 0.0, 1.0)));

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);

        // Process state transitions
        app.update();
        app
    }

    fn spawn_interactable(app: &mut App, pos: Vec2, marker: impl Component) -> Entity {
        app.world_mut()
            .spawn((Interactable, marker, Transform::from_xyz(pos.x, pos.y, 0.0)))
            .with_children(|parent| {
                parent.spawn((
                    HighlightMarker,
                    Sprite {
                        color: Color::srgba(1.0, 1.0, 0.0, 0.5),
                        custom_size: Some(Vec2::splat(40.0 * 1.15)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, -0.1),
                    Visibility::Hidden,
                    Name::new("Highlight"),
                ));
            })
            .id()
    }

    fn get_captured_events(app: &mut App) -> Vec<InteractionEvent> {
        let captured = app.world().resource::<CapturedEvents>();
        captured.0.clone()
    }

    #[test]
    fn space_interacts_with_closest_in_range() {
        let mut app = setup_interaction_app();
        let _far = spawn_interactable(&mut app, Vec2::new(20.0, 0.0), FarmPot);
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
    fn space_does_nothing_while_the_crafting_menu_is_open() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(10.0, 0.0), FarmPot);
        app.world_mut().resource_mut::<CraftingMenu>().open = true;

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn highlights_hide_while_the_crafting_menu_is_open() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(20.0, 0.0), FarmPot);
        app.update();

        let highlight_entity = {
            let mut q = app.world_mut().query::<(Entity, &HighlightMarker)>();
            q.iter(app.world())
                .next()
                .expect("Highlight child not found")
                .0
        };
        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Visible
        );

        app.world_mut().resource_mut::<CraftingMenu>().open = true;
        app.update();

        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Hidden
        );
    }

    #[test]
    fn space_does_nothing_while_the_inventory_is_open() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(10.0, 0.0), FarmPot);
        app.world_mut()
            .resource_mut::<InventoryPanel>()
            .open_panel();

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn highlights_hide_while_the_inventory_is_open() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(20.0, 0.0), FarmPot);
        app.update();

        let highlight_entity = {
            let mut q = app.world_mut().query::<(Entity, &HighlightMarker)>();
            q.iter(app.world())
                .next()
                .expect("Highlight child not found")
                .0
        };
        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Visible
        );

        app.world_mut()
            .resource_mut::<InventoryPanel>()
            .open_panel();
        app.update();

        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Hidden
        );
    }

    #[test]
    fn highlights_come_back_after_the_inventory_closes() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(20.0, 0.0), FarmPot);
        let highlight_entity = {
            let mut q = app.world_mut().query::<(Entity, &HighlightMarker)>();
            q.iter(app.world())
                .next()
                .expect("Highlight child not found")
                .0
        };
        app.world_mut()
            .resource_mut::<InventoryPanel>()
            .open_panel();
        app.update();
        app.world_mut()
            .resource_mut::<InventoryPanel>()
            .close_panel();
        app.update();

        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Visible
        );
    }

    #[test]
    fn space_does_nothing_while_the_crop_picker_is_open() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(10.0, 0.0), FarmPot);
        app.world_mut()
            .resource_mut::<CropSelectMenu>()
            .open_menu(0);

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn highlights_hide_while_the_crop_picker_is_open() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(20.0, 0.0), FarmPot);
        app.update();

        let highlight_entity = {
            let mut q = app.world_mut().query::<(Entity, &HighlightMarker)>();
            q.iter(app.world())
                .next()
                .expect("Highlight child not found")
                .0
        };
        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Visible
        );

        app.world_mut()
            .resource_mut::<CropSelectMenu>()
            .open_menu(0);
        app.update();

        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Hidden
        );
    }

    #[test]
    fn space_only_in_farming_phase() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(10.0, 0.0), FarmPot);

        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossSelect);
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

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::LoadingAssets);
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
        spawn_interactable(&mut app, Vec2::new(20.0, 0.0), FarmPot);
        app.update(); // Allow TransformPropagation

        // Find the highlight entity by marker and check its visibility
        let highlight_entity = {
            let mut q = app.world_mut().query::<(Entity, &HighlightMarker)>();
            q.iter(app.world())
                .next()
                .expect("Highlight child not found")
                .0
        };
        let has_outline = app
            .world()
            .get::<Visibility>(highlight_entity)
            .map(|v| *v == Visibility::Visible)
            .unwrap_or(false);
        assert!(has_outline);
    }

    #[test]
    fn highlight_removes_outline_out_of_range() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(100.0, 0.0), FarmPot);
        app.update(); // Allow TransformPropagation

        let highlight_entity = {
            let mut q = app.world_mut().query::<(Entity, &HighlightMarker)>();
            q.iter(app.world())
                .next()
                .expect("Highlight child not found")
                .0
        };
        let has_outline = app
            .world()
            .get::<Visibility>(highlight_entity)
            .map(|v| *v == Visibility::Visible)
            .unwrap_or(false);
        assert!(!has_outline);
    }

    #[test]
    fn highlight_updates_when_player_moves() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(0.0, 0.0), FarmPot);
        app.update(); // Allow TransformPropagation

        let highlight_entity = {
            let mut q = app.world_mut().query::<(Entity, &HighlightMarker)>();
            q.iter(app.world())
                .next()
                .expect("Highlight child not found")
                .0
        };
        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Visible
        );

        let mut player_transform = *app
            .world_mut()
            .query_filtered::<&mut Transform, With<Player>>()
            .single(app.world_mut())
            .unwrap();
        player_transform.translation.x = 100.0;
        app.world_mut()
            .query_filtered::<&mut Transform, With<Player>>()
            .single_mut(app.world_mut())
            .unwrap()
            .translation = player_transform.translation;
        app.update();

        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Hidden
        );
    }

    #[test]
    fn highlight_only_on_interactables() {
        let mut app = setup_interaction_app();
        let entity = app
            .world_mut()
            .spawn((
                Transform::from_xyz(10.0, 0.0, 0.0),
                GlobalTransform::default(),
            ))
            .id();

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
        let interactable = Interactable;
        let _ = interactable;
    }
}
