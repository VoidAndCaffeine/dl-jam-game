use crate::components::collider::Collider;
use crate::plugins::interaction::{CraftingStation, HighlightMarker, Interactable};
use crate::resources::run_data::PlayerGear;
use crate::states::GameState;
use crate::systems::crafting::{
    CraftingMenuSet, close_menu_on_input, close_menu_outside_farming, craft_row_on_click,
    craft_selected_row, move_menu_selection, open_menu_on_station_interaction,
};
use bevy::prelude::*;

pub const CRAFTING_STATION_SIZE: f32 = 64.0;
pub const CRAFTING_STATION_POS: Vec2 = Vec2::new(-200.0, 0.0);

pub struct GearPlugin;

impl Plugin for GearPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerGear>()
            .add_message::<crate::events::GearCrafted>()
            .add_message::<crate::events::GearEquipped>()
            .add_systems(OnEnter(GameState::Playing), spawn_crafting_station)
            .add_systems(OnExit(GameState::Playing), despawn_crafting_station)
            .add_systems(
                Update,
                (
                    open_menu_on_station_interaction,
                    close_menu_outside_farming,
                    close_menu_on_input,
                    move_menu_selection,
                    craft_selected_row,
                    craft_row_on_click,
                )
                    .chain()
                    .in_set(CraftingMenuSet::Menu),
            );
    }
}

fn spawn_crafting_station(mut commands: Commands) {
    commands
        .spawn((
            Interactable::new(),
            CraftingStation,
            Collider {
                size: Vec2::splat(CRAFTING_STATION_SIZE),
                is_solid: true,
            },
            Sprite {
                color: Color::BLACK,
                custom_size: Some(Vec2::splat(CRAFTING_STATION_SIZE)),
                ..default()
            },
            Transform::from_xyz(CRAFTING_STATION_POS.x, CRAFTING_STATION_POS.y, 0.0),
            Name::new("Crafting Station"),
        ))
        .with_children(|parent| {
            parent.spawn((
                HighlightMarker,
                Sprite {
                    color: Color::srgba(1.0, 1.0, 0.0, 0.5),
                    custom_size: Some(Vec2::splat(CRAFTING_STATION_SIZE * 1.15)),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, -0.1),
                Visibility::Hidden,
                Name::new("Highlight"),
            ));
        });
}

fn despawn_crafting_station(
    mut commands: Commands,
    stations: Query<Entity, With<CraftingStation>>,
) {
    for entity in stations.iter() {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::player::{INTERACTION_RANGE, Player};
    use crate::events::{InteractionEvent, InteractionType};
    use crate::plugins::interaction::InteractionPlugin;
    use crate::resources::crafting_menu::CraftingMenu;
    use crate::resources::inventory::Inventory;
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

    fn setup_gear_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<CapturedEvents>()
            .init_resource::<CraftingMenu>()
            .init_resource::<Inventory>()
            .add_plugins((
                MinimalPlugins,
                TransformPlugin,
                StatesPlugin,
                InteractionPlugin,
                GearPlugin,
            ))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_systems(Update, capture_events.after(CraftingMenuSet::Menu));

        app.world_mut()
            .spawn((Player, Transform::from_xyz(0.0, 0.0, 1.0)));

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);
        app.update();
        app
    }

    /// Presses a key and runs two frames so the capture system is guaranteed to
    /// observe the message regardless of how the systems happen to be ordered.
    fn press_key(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.update();
    }

    fn move_player(app: &mut App, pos: Vec2) {
        let mut query = app
            .world_mut()
            .query_filtered::<&mut Transform, With<Player>>();
        let mut transform = query.single_mut(app.world_mut()).unwrap();
        transform.translation.x = pos.x;
        transform.translation.y = pos.y;
    }

    fn station_entity(app: &mut App) -> Entity {
        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<CraftingStation>>();
        query
            .iter(app.world())
            .next()
            .expect("Crafting station not spawned")
    }

    #[test]
    fn gear_plugin_exists() {
        let _plugin = GearPlugin;
    }

    #[test]
    fn crafting_station_spawns_with_expected_components() {
        let mut app = setup_gear_app();
        let entity = station_entity(&mut app);

        assert!(app.world().get::<Interactable>(entity).is_some());

        let collider = app
            .world()
            .get::<Collider>(entity)
            .expect("Station must have a collider");
        assert!(collider.is_solid);
        assert_eq!(collider.size, Vec2::splat(CRAFTING_STATION_SIZE));

        let sprite = app
            .world()
            .get::<Sprite>(entity)
            .expect("Station needs a sprite");
        assert_eq!(sprite.color, Color::BLACK);
        assert_eq!(sprite.custom_size, Some(Vec2::splat(CRAFTING_STATION_SIZE)));

        let transform = app.world().get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation.truncate(), CRAFTING_STATION_POS);
    }

    #[test]
    fn crafting_station_has_hidden_highlight_child() {
        let mut app = setup_gear_app();
        let entity = station_entity(&mut app);

        let child_entities: Vec<Entity> = {
            let mut query = app.world_mut().query::<&Children>();
            query
                .get_mut(app.world_mut(), entity)
                .map(|children| children.iter().collect())
                .unwrap_or_default()
        };
        assert_eq!(child_entities.len(), 1, "station should have one child");

        let highlight = child_entities
            .into_iter()
            .find(|child| app.world().get::<HighlightMarker>(*child).is_some())
            .expect("Highlight child not found");

        assert_eq!(
            app.world().get::<Visibility>(highlight).unwrap(),
            &Visibility::Hidden
        );
        assert_eq!(
            app.world()
                .get::<Transform>(highlight)
                .unwrap()
                .translation
                .z,
            -0.1
        );
    }

    #[test]
    fn crafting_station_is_opposite_side_from_pots() {
        let mut app = setup_gear_app();
        let entity = station_entity(&mut app);
        let transform = app.world().get::<Transform>(entity).unwrap();

        assert!(
            transform.translation.x < 0.0,
            "station should sit on the negative x side of spawn"
        );
        assert!(transform.translation.x + CRAFTING_STATION_SIZE * 0.5 < 0.0);
    }

    #[test]
    fn space_near_station_emits_crafting_interaction() {
        let mut app = setup_gear_app();
        let entity = station_entity(&mut app);
        app.update(); // allow TransformPropagation to compute GlobalTransform

        move_player(
            &mut app,
            CRAFTING_STATION_POS + Vec2::new(INTERACTION_RANGE * 0.5, 0.0),
        );
        app.update();

        press_key(&mut app, KeyCode::Space);

        let captured = app.world().resource::<CapturedEvents>();
        assert_eq!(captured.0.len(), 1);
        assert_eq!(captured.0[0].entity, entity);
        assert_eq!(captured.0[0].interaction_type, InteractionType::Crafting);
    }

    #[test]
    fn space_far_from_station_emits_nothing() {
        let mut app = setup_gear_app();
        station_entity(&mut app);
        app.update();

        move_player(&mut app, Vec2::new(200.0, 0.0));
        app.update();

        press_key(&mut app, KeyCode::Space);

        let captured = app.world().resource::<CapturedEvents>();
        assert_eq!(captured.0.len(), 0);
    }

    #[test]
    fn interacting_with_the_station_opens_the_crafting_menu() {
        let mut app = setup_gear_app();
        station_entity(&mut app);
        app.update();

        move_player(
            &mut app,
            CRAFTING_STATION_POS + Vec2::new(INTERACTION_RANGE * 0.5, 0.0),
        );
        app.update();

        assert!(!app.world().resource::<CraftingMenu>().open);
        press_key(&mut app, KeyCode::Space);
        assert!(app.world().resource::<CraftingMenu>().open);
    }

    #[test]
    fn escape_closes_the_crafting_menu() {
        let mut app = setup_gear_app();
        station_entity(&mut app);
        app.update();
        move_player(
            &mut app,
            CRAFTING_STATION_POS + Vec2::new(INTERACTION_RANGE * 0.5, 0.0),
        );
        app.update();
        press_key(&mut app, KeyCode::Space);
        assert!(app.world().resource::<CraftingMenu>().open);

        press_key(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<CraftingMenu>().open);
    }

    #[test]
    fn station_despawns_on_exit_playing() {
        let mut app = setup_gear_app();
        assert!(
            app.world_mut()
                .query_filtered::<Entity, With<CraftingStation>>()
                .iter(app.world())
                .next()
                .is_some()
        );

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();

        assert!(
            app.world_mut()
                .query_filtered::<Entity, With<CraftingStation>>()
                .iter(app.world())
                .next()
                .is_none(),
            "station should be despawned when leaving Playing"
        );
    }
}
