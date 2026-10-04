use bevy::prelude::*;

use crate::plugins::{FarmPlugin, GearPlugin, InteractionPlugin, UIPlugin};
use crate::resources::camera::CameraFollowConfig;
use crate::resources::crafting_menu::CraftingMenu;
use crate::resources::inventory::Inventory;
use crate::resources::run_data::PlayerGear;
use crate::states::{DayPhase, GameState};
use crate::systems::camera_follow::camera_follow;
use crate::systems::collision::collision_detection;
use crate::systems::collision_response::collision_response;
use crate::systems::movement_input::movement_input;
use crate::systems::movement_physics::movement_physics;
use crate::systems::spawn_player::{despawn_player, spawn_player};
use crate::systems::transition::transition_to_playing;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_state::<DayPhase>()
            .init_resource::<CameraFollowConfig>()
            .init_resource::<CraftingMenu>()
            .init_resource::<Inventory>()
            .init_resource::<PlayerGear>()
            .add_plugins(InteractionPlugin)
            .add_plugins(FarmPlugin)
            .add_plugins(GearPlugin)
            .add_plugins(UIPlugin)
            .add_systems(OnEnter(GameState::LoadingAssets), transition_to_playing)
            .add_systems(OnEnter(GameState::Playing), spawn_player)
            .add_systems(OnExit(GameState::Playing), despawn_player)
            .add_systems(FixedUpdate, movement_input)
            .add_systems(FixedUpdate, movement_physics)
            .add_systems(FixedUpdate, collision_detection)
            .add_systems(FixedUpdate, camera_follow)
            .add_observer(collision_response);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::gear::{GearSet, GearSlot};
    use crate::components::pot::CropType;
    use crate::events::{InteractionEvent, InteractionType};
    use crate::plugins::interaction::CraftingStation;
    use crate::plugins::ui::CraftingMenuRoot;
    use crate::resources::crafting_menu::CraftingMenu;
    use crate::resources::inventory::Inventory;
    use crate::resources::run_data::PlayerGear;
    use crate::systems::crafting::RecipeRow;
    use bevy::state::app::StatesPlugin;
    use bevy::transform::TransformPlugin;

    fn setup_game_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin, GamePlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>();
        app
    }

    fn enter_playing(app: &mut App) {
        for _ in 0..3 {
            app.update();
        }
    }

    fn tap_key(app: &mut App, key: KeyCode) {
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

    fn menu(app: &App) -> &CraftingMenu {
        app.world().resource::<CraftingMenu>()
    }

    fn gear(app: &App) -> &PlayerGear {
        app.world().resource::<PlayerGear>()
    }

    fn station_interaction(app: &mut App) {
        let station = app
            .world_mut()
            .query_filtered::<Entity, With<CraftingStation>>()
            .iter(app.world())
            .next()
            .expect("crafting station is spawned");
        app.world_mut().write_message(InteractionEvent {
            entity: station,
            interaction_type: InteractionType::Crafting,
        });
        app.update();
    }

    fn menu_roots(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<CraftingMenuRoot>>()
            .iter(app.world())
            .collect()
    }

    fn menu_rows(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<RecipeRow>>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn game_starts_in_playing_with_the_farm_and_station() {
        let mut app = setup_game_app();
        enter_playing(&mut app);

        assert_eq!(
            app.world().resource::<State<GameState>>().get(),
            &GameState::Playing
        );
        assert!(
            app.world_mut()
                .query_filtered::<Entity, With<crate::components::pot::Pot>>()
                .iter(app.world())
                .next()
                .is_some()
        );
        assert_eq!(menu(&app).open, false);
        assert!(menu_roots(&mut app).is_empty());
    }

    #[test]
    fn opening_and_using_the_menu_works_end_to_end() {
        let mut app = setup_game_app();
        enter_playing(&mut app);

        app.world_mut()
            .resource_mut::<Inventory>()
            .add_crop(CropType::Starter, 10);

        station_interaction(&mut app);
        assert!(menu(&app).open);
        assert_eq!(menu_roots(&mut app).len(), 1);
        assert_eq!(menu_rows(&mut app), 4);

        tap_key(&mut app, KeyCode::Enter);
        assert_eq!(menu(&app).notice, "Crafted Starter Set");
        assert!(gear(&app).owns_set(GearSet::Starter));
        assert_eq!(
            gear(&app).equipped(GearSlot::Weapon),
            Some(crate::components::gear::GearPiece::new(
                GearSet::Starter,
                GearSlot::Weapon
            ))
        );

        assert_eq!(menu_rows(&mut app), 6, "both owned pieces get rows");

        tap_key(&mut app, KeyCode::Escape);
        assert!(!menu(&app).open);
        assert!(menu_roots(&mut app).is_empty());
    }

    #[test]
    fn menu_survives_being_reopened_and_refreshes_counts() {
        let mut app = setup_game_app();
        enter_playing(&mut app);

        station_interaction(&mut app);
        tap_key(&mut app, KeyCode::Tab);
        assert!(!menu(&app).open);

        app.world_mut()
            .resource_mut::<Inventory>()
            .add_crop(CropType::Starter, 12);
        station_interaction(&mut app);
        assert!(menu(&app).open);

        let mut texts: Vec<String> = app
            .world_mut()
            .query_filtered::<&Text, With<bevy::prelude::Node>>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        texts.sort();
        assert!(
            texts
                .iter()
                .any(|text| text.contains("Starter Crop 12") && text.starts_with("Inventory:")),
            "inventory line missing from {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|text| text.contains("Starter Crop 12/10") && text.contains("Craft")),
            "craftable row status missing from {texts:?}"
        );
    }

    #[test]
    fn leaving_playing_clears_the_menu_state() {
        let mut app = setup_game_app();
        enter_playing(&mut app);
        station_interaction(&mut app);
        assert!(menu(&app).open);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();

        assert!(!menu(&app).open);
        assert!(menu_roots(&mut app).is_empty());
    }

    #[test]
    fn running_frames_with_the_menu_open_keeps_it_stable() {
        let mut app = setup_game_app();
        enter_playing(&mut app);
        station_interaction(&mut app);
        let root = menu_roots(&mut app)[0];

        for _ in 0..20 {
            app.update();
        }

        assert_eq!(menu_roots(&mut app), vec![root]);
        assert_eq!(menu_rows(&mut app), 4);
        assert_eq!(menu(&app).selected, 0);
    }
}
