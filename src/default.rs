use bevy::prelude::*;

use crate::plugins::{
    BossPlugin, DayCyclePlugin, FarmPlugin, GearPlugin, InteractionPlugin, LevelPlugin, UIPlugin,
};
use crate::resources::camera::CameraFollowConfig;
use crate::resources::crafting_menu::CraftingMenu;
use crate::resources::inventory::Inventory;
use crate::resources::inventory_panel::InventoryPanel;
use crate::resources::level::LevelSet;
use crate::resources::run_data::PlayerGear;
use crate::states::{DayPhase, GameState};
use crate::systems::boss_select::BossSelectPlugin;
use crate::systems::camera_follow::camera_follow;
use crate::systems::crop_select::CropSelectPlugin;
use crate::systems::level_movement::grid_movement;
use crate::systems::movement_input::movement_input;
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
            .init_resource::<InventoryPanel>()
            .init_resource::<PlayerGear>()
            .add_plugins(InteractionPlugin)
            .add_plugins(LevelPlugin)
            .add_plugins(FarmPlugin)
            .add_plugins(GearPlugin)
            .add_plugins(BossSelectPlugin)
            .add_plugins(CropSelectPlugin)
            .add_plugins(BossPlugin)
            .add_plugins(DayCyclePlugin)
            .add_plugins(UIPlugin)
            .insert_resource(Time::<Fixed>::from_hz(60.0))
            .add_systems(OnEnter(GameState::LoadingAssets), transition_to_playing)
            .add_systems(
                OnEnter(GameState::Playing),
                spawn_player.after(LevelSet::Load),
            )
            .add_systems(OnExit(GameState::Playing), despawn_player)
            .add_systems(FixedUpdate, movement_input)
            .add_systems(FixedUpdate, grid_movement.after(movement_input))
            .add_systems(FixedUpdate, camera_follow);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::gear::{GearPiece, GearSet, GearSlot, RECIPE_COUNT};
    use crate::components::pot::{CropType, Pot, PotState};
    use crate::events::{InteractionEvent, InteractionType};
    use crate::plugins::interaction::CraftingStation;
    use crate::plugins::ui::{CraftingMenuRoot, CropSelectRoot, InventoryRoot};
    use crate::resources::crafting_menu::CraftingMenu;
    use crate::resources::crop_select::CropSelectMenu;
    use crate::resources::farm::CropUnlocks;
    use crate::resources::inventory::Inventory;
    use crate::resources::inventory_panel::InventoryPanel;
    use crate::resources::run_data::PlayerGear;
    use crate::systems::crafting::RecipeRow;
    use crate::systems::inventory::InventorySlot;
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

    fn inventory(app: &App) -> &InventoryPanel {
        app.world().resource::<InventoryPanel>()
    }

    fn inventory_roots(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<InventoryRoot>>()
            .iter(app.world())
            .collect()
    }

    fn inventory_slots(app: &mut App) -> Vec<(usize, Entity)> {
        let mut slots: Vec<(usize, Entity)> = app
            .world_mut()
            .query_filtered::<(Entity, &InventorySlot), With<Button>>()
            .iter(app.world())
            .map(|(entity, slot)| (slot.index, entity))
            .collect();
        slots.sort_by_key(|(index, _)| *index);
        slots
    }

    fn set_day_phase(app: &mut App, phase: DayPhase) {
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(phase);
        app.update();
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
        assert!(!menu(&app).open);
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
        assert_eq!(menu_rows(&mut app), RECIPE_COUNT);

        tap_key(&mut app, KeyCode::Enter);
        assert_eq!(menu(&app).notice, "Crafted Starter Spearblade");
        assert!(gear(&app).owns(&GearPiece::new(GearSet::Starter, GearSlot::Weapon)));

        app.world_mut().resource_mut::<CraftingMenu>().selected = 1;
        tap_key(&mut app, KeyCode::Enter);
        assert!(gear(&app).owns_set(GearSet::Starter));
        assert_eq!(
            gear(&app).equipped(GearSlot::Weapon),
            Some(GearPiece::new(GearSet::Starter, GearSlot::Weapon))
        );
        assert_eq!(
            gear(&app).equipped(GearSlot::Armor),
            Some(GearPiece::new(GearSet::Starter, GearSlot::Armor))
        );

        assert_eq!(
            menu_rows(&mut app),
            RECIPE_COUNT + 2,
            "both owned pieces get rows"
        );

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
                .any(|text| text.contains("Starter Crop 12/2") && text.contains("Craft")),
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
        assert_eq!(menu_rows(&mut app), RECIPE_COUNT);
        assert_eq!(menu(&app).selected, 0);
    }

    #[test]
    fn the_inventory_is_closed_until_the_player_asks_for_it() {
        let mut app = setup_game_app();
        enter_playing(&mut app);
        assert!(!inventory(&app).open);
        assert!(inventory_roots(&mut app).is_empty());
    }

    #[test]
    fn i_key_opens_and_closes_the_inventory_end_to_end() {
        let mut app = setup_game_app();
        enter_playing(&mut app);
        app.world_mut()
            .resource_mut::<Inventory>()
            .add_crop(CropType::Starter, 5);

        tap_key(&mut app, KeyCode::KeyI);
        assert!(inventory(&app).open);
        assert_eq!(inventory_roots(&mut app).len(), 1);
        let slots = inventory_slots(&mut app);
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].0, 0);

        tap_key(&mut app, KeyCode::KeyI);
        assert!(!inventory(&app).open);
        assert!(inventory_roots(&mut app).is_empty());
    }

    #[test]
    fn the_inventory_shows_harvested_crops() {
        let mut app = setup_game_app();
        enter_playing(&mut app);
        app.world_mut()
            .write_message(crate::events::CropHarvested(CropType::Starter));
        app.update();
        app.update();
        tap_key(&mut app, KeyCode::KeyI);

        let texts: Vec<String> = app
            .world_mut()
            .query_filtered::<&Text, With<Node>>()
            .iter(app.world())
            .map(|text| text.0.clone())
            .collect();
        assert!(
            texts.iter().any(|text| text == "Starter Crop"),
            "crop row missing from {texts:?}"
        );
        assert!(
            texts.iter().any(|text| text == "x1"),
            "crop count missing from {texts:?}"
        );
    }

    #[test]
    fn crafting_then_equipping_from_the_inventory_works_end_to_end() {
        let mut app = setup_game_app();
        enter_playing(&mut app);
        app.world_mut()
            .resource_mut::<Inventory>()
            .add_crop(CropType::Starter, 30);

        station_interaction(&mut app);
        tap_key(&mut app, KeyCode::Enter);
        assert!(gear(&app).owns(&GearPiece::new(GearSet::Starter, GearSlot::Weapon)));

        tap_key(&mut app, KeyCode::Escape);
        assert!(!menu(&app).open);

        app.world_mut()
            .resource_mut::<PlayerGear>()
            .own(GearPiece::new(GearSet::Master, GearSlot::Weapon));
        tap_key(&mut app, KeyCode::KeyI);
        assert!(inventory(&app).open);

        let last_slot = inventory_slots(&mut app)
            .pop()
            .expect("the master weapon has a row")
            .0;
        app.world_mut().resource_mut::<InventoryPanel>().selected = last_slot;
        app.update();
        tap_key(&mut app, KeyCode::Enter);

        assert_eq!(
            gear(&app).equipped(GearSlot::Weapon),
            Some(GearPiece::new(GearSet::Master, GearSlot::Weapon))
        );
        assert_eq!(
            inventory(&app).notice,
            "Equipped Master Spearblade (Master Set)"
        );
    }

    #[test]
    fn the_inventory_opens_outside_the_farming_phase() {
        let mut app = setup_game_app();
        enter_playing(&mut app);
        set_day_phase(&mut app, DayPhase::BossSelect);

        tap_key(&mut app, KeyCode::KeyI);
        assert!(inventory(&app).open);

        set_day_phase(&mut app, DayPhase::BossFight);
        assert!(inventory(&app).open);
        assert_eq!(inventory_roots(&mut app).len(), 1);

        tap_key(&mut app, KeyCode::Escape);
        assert!(!inventory(&app).open);
    }

    #[test]
    fn the_crafting_menu_and_the_inventory_never_share_the_screen() {
        let mut app = setup_game_app();
        enter_playing(&mut app);
        station_interaction(&mut app);
        assert_eq!(menu_roots(&mut app).len(), 1);

        tap_key(&mut app, KeyCode::KeyI);
        assert!(menu_roots(&mut app).is_empty());
        assert_eq!(inventory_roots(&mut app).len(), 1);

        app.world_mut().resource_mut::<CraftingMenu>().open_menu();
        app.update();
        assert!(menu_roots(&mut app).is_empty());
        assert_eq!(inventory_roots(&mut app).len(), 1);
    }

    #[test]
    fn leaving_playing_clears_the_inventory_state() {
        let mut app = setup_game_app();
        enter_playing(&mut app);
        tap_key(&mut app, KeyCode::KeyI);
        assert!(inventory(&app).open);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();

        assert!(!inventory(&app).open);
        assert!(inventory_roots(&mut app).is_empty());
    }

    fn crop_roots(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<CropSelectRoot>>()
            .iter(app.world())
            .collect()
    }

    #[test]
    fn planting_from_the_crop_picker_works_end_to_end() {
        let mut app = setup_game_app();
        enter_playing(&mut app);

        app.world_mut()
            .resource_mut::<CropUnlocks>()
            .unlock_crop_a();
        let pot = app
            .world_mut()
            .query_filtered::<(Entity, &Pot), With<Pot>>()
            .iter(app.world())
            .find(|(_, pot)| pot.index == 0)
            .map(|(entity, _)| entity)
            .expect("pot 0 is spawned");

        app.world_mut()
            .resource_mut::<CropSelectMenu>()
            .open_menu(0);
        app.world_mut().resource_mut::<CropSelectMenu>().selected = 1;
        app.update();
        assert_eq!(crop_roots(&mut app).len(), 1);

        tap_key(&mut app, KeyCode::Enter);

        let planted = app.world().get::<Pot>(pot).expect("pot still exists");
        assert_eq!(planted.state, PotState::Planted);
        assert_eq!(planted.crop_type, CropType::CropA);
        assert!(!app.world().resource::<CropSelectMenu>().open);
        assert!(crop_roots(&mut app).is_empty());
    }
}
