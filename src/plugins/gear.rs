use crate::components::collider::Collider;
use crate::plugins::interaction::{CraftingStation, Interactable};
use crate::plugins::prop::PropArt;
use crate::resources::run_data::PlayerGear;
use crate::states::GameState;
use crate::systems::crafting::{
    CraftingMenuSet, close_menu_on_input, close_menu_outside_farming, craft_row_on_click,
    craft_selected_row, move_menu_selection, open_menu_on_station_interaction,
};
use bevy::prelude::*;

pub const CRAFTING_STATION_SIZE: f32 = 64.0;

pub struct GearPlugin;

impl Plugin for GearPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlayerGear>()
            .add_message::<crate::events::GearCrafted>()
            .add_message::<crate::events::GearEquipped>()
            .add_message::<crate::events::PlaySfx>()
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

/// Spawns the crafting station at a world position. The position comes from the
/// level's `s` marker.
///
/// Only the gameplay components are attached here; [`crate::plugins::prop::PropPlugin`]
/// draws the station's art and interaction outline once the renderer is up.
pub fn spawn_crafting_station(commands: &mut Commands, position: Vec2) -> Entity {
    commands
        .spawn((
            Interactable::new(),
            CraftingStation,
            PropArt::CraftingStation,
            Collider {
                size: Vec2::splat(CRAFTING_STATION_SIZE),
                is_solid: true,
            },
            Transform::from_xyz(position.x, position.y, 0.0),
            Name::new("Crafting Station"),
        ))
        .id()
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
    use crate::plugins::level::LevelPlugin;
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
                LevelPlugin,
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

    /// Presses a key for exactly one frame, then runs a second frame so the
    /// capture system observes the message regardless of system ordering.
    ///
    /// Nothing clears `just_pressed` without the input plugin, so it has to be
    /// cleared by hand or the interaction fires on both frames.
    fn press_key(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear_just_pressed(key);
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

    /// A spot inside the farm that is out of interaction range of everything.
    fn far_corner(_app: &App) -> Vec2 {
        Vec2::new(-1000.0, -1000.0)
    }

    fn station_position(app: &mut App) -> Vec2 {
        app.world_mut()
            .query_filtered::<&Transform, With<CraftingStation>>()
            .single(app.world())
            .expect("Crafting station not spawned")
            .translation
            .truncate()
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
        assert!(app.world().get::<CraftingStation>(entity).is_some());
        assert_eq!(
            app.world().get::<PropArt>(entity),
            Some(&PropArt::CraftingStation)
        );

        let collider = app
            .world()
            .get::<Collider>(entity)
            .expect("Station must have a collider");
        assert!(collider.is_solid);
        assert_eq!(collider.size, Vec2::splat(CRAFTING_STATION_SIZE));

        let actual = app
            .world()
            .get::<Transform>(entity)
            .unwrap()
            .translation
            .truncate();
        assert_eq!(actual, station_position(&mut app));
    }

    #[test]
    fn crafting_station_draws_its_outline_through_a_material_not_a_child() {
        let mut app = setup_gear_app();
        let entity = station_entity(&mut app);
        app.update();

        // The shared outline material draws the highlight around the station's
        // own sprite, so no separate highlight child is spawned (this matches the
        // pot mounds). The material itself only attaches under a renderer.
        assert!(
            app.world().get::<Children>(entity).is_none(),
            "the station should not carry a highlight child"
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

        let beside = station_position(&mut app) + Vec2::new(INTERACTION_RANGE * 0.5, 0.0);
        move_player(&mut app, beside);
        app.update();

        press_key(&mut app, KeyCode::Space);

        let captured = app.world().resource::<CapturedEvents>();
        assert_eq!(captured.0.len(), 1, "captured {:?}", captured.0);
        assert_eq!(captured.0[0].entity, entity);
        assert_eq!(captured.0[0].interaction_type, InteractionType::Crafting);
    }

    #[test]
    fn space_far_from_station_emits_nothing() {
        let mut app = setup_gear_app();
        station_entity(&mut app);
        app.update();

        let away = far_corner(&app);
        move_player(&mut app, away);
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

        let beside = station_position(&mut app) + Vec2::new(INTERACTION_RANGE * 0.5, 0.0);
        move_player(&mut app, beside);
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
        let beside = station_position(&mut app) + Vec2::new(INTERACTION_RANGE * 0.5, 0.0);
        move_player(&mut app, beside);
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
