use crate::components::gear::{
    GearPiece, GearSet, ItemCost, RECIPE_COUNT, RECIPES, recipe_for_set,
};
use crate::events::{GearCrafted, GearEquipped, InteractionEvent, InteractionType};
use crate::resources::crafting_menu::CraftingMenu;
use crate::resources::inventory::Inventory;
use crate::resources::run_data::PlayerGear;
use crate::states::{DayPhase, GameState};
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::prelude::*;
use bevy::state::state::State;

pub const OWNED_ROW_OFFSET: usize = RECIPE_COUNT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowAction {
    Craft(GearSet),
    Equip(GearPiece),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CraftOutcome {
    Crafted { set: GearSet },
    Equipped { piece: GearPiece },
    AlreadyEquipped { piece: GearPiece },
    AlreadyOwned { set: GearSet },
    Missing { items: Vec<(ItemCost, u32)> },
}

impl CraftOutcome {
    pub fn notice(&self) -> String {
        match self {
            CraftOutcome::Crafted { set } => format!("Crafted {}", set.label()),
            CraftOutcome::Equipped { piece } => format!("Equipped {}", piece.describe()),
            CraftOutcome::AlreadyEquipped { piece } => {
                format!("{} is already equipped", piece.name())
            }
            CraftOutcome::AlreadyOwned { set } => format!("{} already owned", set.label()),
            CraftOutcome::Missing { items } => {
                let parts: Vec<String> = items
                    .iter()
                    .map(|(item, amount)| format!("{} x{}", item.label(), amount))
                    .collect();
                format!("Missing: {}", parts.join(", "))
            }
        }
    }
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CraftingMenuSet {
    Menu,
}

#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct RecipeRow {
    pub index: usize,
}

pub fn row_count(owned: &[GearPiece]) -> usize {
    OWNED_ROW_OFFSET + owned.len()
}

pub fn row_action(row: usize, owned: &[GearPiece]) -> Option<RowAction> {
    if row < OWNED_ROW_OFFSET {
        let recipe = RECIPES.get(row)?;
        return Some(RowAction::Craft(recipe.set));
    }
    owned
        .get(row - OWNED_ROW_OFFSET)
        .map(|piece| RowAction::Equip(*piece))
}

pub fn perform(
    action: RowAction,
    inventory: &mut Inventory,
    gear: &mut PlayerGear,
) -> CraftOutcome {
    match action {
        RowAction::Craft(set) => {
            if gear.owns_set(set) {
                return CraftOutcome::AlreadyOwned { set };
            }
            let recipe = recipe_for_set(set);
            if !inventory.can_craft(recipe) {
                return CraftOutcome::Missing {
                    items: inventory.missing(recipe),
                };
            }
            inventory.consume(recipe);
            for piece in recipe.pieces() {
                gear.own(piece);
                gear.equip(&piece);
            }
            CraftOutcome::Crafted { set }
        }
        RowAction::Equip(piece) => {
            if gear.is_equipped(&piece) {
                return CraftOutcome::AlreadyEquipped { piece };
            }
            if gear.equip(&piece) {
                CraftOutcome::Equipped { piece }
            } else {
                CraftOutcome::AlreadyEquipped { piece }
            }
        }
    }
}

/// Applies a row action, writing the resulting craft/equip messages. Callers own
/// their own notice so several panels can share this logic.
pub fn apply(
    action: RowAction,
    inventory: &mut Inventory,
    gear: &mut PlayerGear,
    crafted_events: &mut MessageWriter<GearCrafted>,
    equipped_events: &mut MessageWriter<GearEquipped>,
) -> CraftOutcome {
    let outcome = perform(action, inventory, gear);
    match &outcome {
        CraftOutcome::Crafted { set } => {
            for piece in recipe_for_set(*set).pieces() {
                crafted_events.write(GearCrafted(piece));
                equipped_events.write(GearEquipped(piece.slot));
            }
        }
        CraftOutcome::Equipped { piece } => {
            equipped_events.write(GearEquipped(piece.slot));
        }
        _ => {}
    }
    outcome
}

fn menu_active(
    game_state: &State<GameState>,
    day_phase: &State<DayPhase>,
    menu: &CraftingMenu,
) -> bool {
    matches!(game_state.get(), GameState::Playing)
        && matches!(day_phase.get(), DayPhase::Farming)
        && menu.open
}

pub fn open_menu_on_station_interaction(
    mut events: MessageReader<InteractionEvent>,
    mut menu: ResMut<CraftingMenu>,
    game_state: Res<State<GameState>>,
    day_phase: Res<State<DayPhase>>,
) {
    if !matches!(game_state.get(), GameState::Playing)
        || !matches!(day_phase.get(), DayPhase::Farming)
    {
        return;
    }
    for event in events.read() {
        if event.interaction_type == InteractionType::Crafting {
            menu.open_menu();
        }
    }
}

pub fn close_menu_on_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<CraftingMenu>,
    game_state: Res<State<GameState>>,
    day_phase: Res<State<DayPhase>>,
) {
    if !menu_active(&game_state, &day_phase, &menu) {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::Tab) {
        menu.close_menu();
    }
}

pub fn close_menu_outside_farming(
    mut menu: ResMut<CraftingMenu>,
    game_state: Res<State<GameState>>,
    day_phase: Res<State<DayPhase>>,
) {
    if menu.open && !menu_active(&game_state, &day_phase, &menu) {
        menu.close_menu();
    }
}

pub fn move_menu_selection(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<CraftingMenu>,
    gear: Res<PlayerGear>,
    game_state: Res<State<GameState>>,
    day_phase: Res<State<DayPhase>>,
) {
    if !menu_active(&game_state, &day_phase, &menu) {
        return;
    }
    let last_row = row_count(&gear.owned).saturating_sub(1);
    let next = if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) {
        Some(menu.selected + 1)
    } else if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) {
        Some(menu.selected.saturating_sub(1))
    } else {
        None
    };
    let Some(next) = next else {
        return;
    };
    menu.selected = next.min(last_row);
}

pub fn craft_selected_row(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<CraftingMenu>,
    mut inventory: ResMut<Inventory>,
    mut gear: ResMut<PlayerGear>,
    mut crafted_events: MessageWriter<GearCrafted>,
    mut equipped_events: MessageWriter<GearEquipped>,
    game_state: Res<State<GameState>>,
    day_phase: Res<State<DayPhase>>,
) {
    if !menu_active(&game_state, &day_phase, &menu) {
        return;
    }
    if !keys.just_pressed(KeyCode::Enter) && !keys.just_pressed(KeyCode::KeyE) {
        return;
    }
    let Some(action) = row_action(menu.selected, &gear.owned) else {
        return;
    };
    let outcome = apply(
        action,
        &mut inventory,
        &mut gear,
        &mut crafted_events,
        &mut equipped_events,
    );
    menu.set_notice(outcome.notice());
}

pub fn craft_row_on_click(
    rows: Query<(Entity, &RecipeRow), Changed<Interaction>>,
    interactions: Query<&Interaction>,
    mut menu: ResMut<CraftingMenu>,
    mut inventory: ResMut<Inventory>,
    mut gear: ResMut<PlayerGear>,
    mut crafted_events: MessageWriter<GearCrafted>,
    mut equipped_events: MessageWriter<GearEquipped>,
    game_state: Res<State<GameState>>,
    day_phase: Res<State<DayPhase>>,
) {
    if !menu_active(&game_state, &day_phase, &menu) {
        return;
    }
    for (entity, row) in rows.iter() {
        let Ok(interaction) = interactions.get(entity) else {
            continue;
        };
        if *interaction != Interaction::Pressed {
            continue;
        }
        menu.selected = row.index;
        let Some(action) = row_action(row.index, &gear.owned) else {
            continue;
        };
        let outcome = apply(
            action,
            &mut inventory,
            &mut gear,
            &mut crafted_events,
            &mut equipped_events,
        );
        menu.set_notice(outcome.notice());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::gear::{GearSlot, MaterialType, RECIPES};
    use crate::components::pot::CropType;
    use bevy::ecs::message::MessageReader;
    use bevy::state::app::StatesPlugin;
    use bevy::transform::TransformPlugin;

    #[derive(Resource, Default)]
    struct CapturedCrafted(Vec<GearPiece>);

    #[derive(Resource, Default)]
    struct CapturedEquipped(Vec<GearSlot>);

    fn capture_crafted(
        mut captured: ResMut<CapturedCrafted>,
        mut reader: MessageReader<GearCrafted>,
    ) {
        for event in reader.read() {
            captured.0.push(event.0);
        }
    }

    fn capture_equipped(
        mut captured: ResMut<CapturedEquipped>,
        mut reader: MessageReader<GearEquipped>,
    ) {
        for event in reader.read() {
            captured.0.push(event.0);
        }
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<CraftingMenu>()
            .init_resource::<Inventory>()
            .init_resource::<PlayerGear>()
            .init_resource::<CapturedCrafted>()
            .init_resource::<CapturedEquipped>()
            .add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_message::<InteractionEvent>()
            .add_message::<GearCrafted>()
            .add_message::<GearEquipped>()
            .add_systems(
                Update,
                (
                    open_menu_on_station_interaction,
                    close_menu_on_input,
                    close_menu_outside_farming,
                    move_menu_selection,
                    craft_selected_row,
                    craft_row_on_click,
                )
                    .chain()
                    .in_set(CraftingMenuSet::Menu),
            )
            .add_systems(
                Update,
                (
                    capture_crafted.after(CraftingMenuSet::Menu),
                    capture_equipped.after(CraftingMenuSet::Menu),
                ),
            );

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);
        app.update();
        app
    }

    fn open_menu(app: &mut App) {
        app.world_mut().resource_mut::<CraftingMenu>().open_menu();
    }

    fn select(app: &mut App, row: usize) {
        app.world_mut().resource_mut::<CraftingMenu>().selected = row;
    }

    /// Releases any earlier press, presses the key for one frame, then drops the
    /// just-pressed state so the key can be pressed again by a later call.
    fn press(app: &mut App, key: KeyCode) {
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

    fn inventory(app: &App) -> &Inventory {
        app.world().resource::<Inventory>()
    }

    fn gear(app: &App) -> &PlayerGear {
        app.world().resource::<PlayerGear>()
    }

    fn grant_starter_crops(app: &mut App, amount: u32) {
        app.world_mut()
            .resource_mut::<Inventory>()
            .add_crop(CropType::Starter, amount);
    }

    fn crafted(app: &App) -> Vec<GearPiece> {
        app.world().resource::<CapturedCrafted>().0.clone()
    }

    fn equipped(app: &App) -> Vec<GearSlot> {
        app.world().resource::<CapturedEquipped>().0.clone()
    }

    fn station_interaction(app: &mut App) {
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(InteractionEvent {
            entity,
            interaction_type: InteractionType::Crafting,
        });
        app.update();
    }

    #[test]
    fn recipe_rows_map_to_craft_actions() {
        let owned: Vec<GearPiece> = Vec::new();
        for (index, recipe) in RECIPES.iter().enumerate() {
            assert_eq!(
                row_action(index, &owned),
                Some(RowAction::Craft(recipe.set))
            );
        }
    }

    #[test]
    fn owned_rows_map_to_equip_actions() {
        let owned = vec![GearPiece::new(GearSet::Starter, GearSlot::Weapon)];
        assert_eq!(row_count(&owned), RECIPE_COUNT + 1);
        assert_eq!(
            row_action(OWNED_ROW_OFFSET, &owned),
            Some(RowAction::Equip(owned[0]))
        );
        assert_eq!(row_action(OWNED_ROW_OFFSET + 1, &owned), None);
    }

    #[test]
    fn row_action_out_of_range_is_none() {
        assert_eq!(row_action(RECIPE_COUNT, &[]), None);
        assert_eq!(row_action(99, &[]), None);
    }

    #[test]
    fn perform_craft_consumes_owns_and_equips_both_pieces() {
        let mut inventory = Inventory::default();
        inventory.add_crop(CropType::Starter, 10);
        let mut gear = PlayerGear::default();

        let outcome = perform(
            RowAction::Craft(GearSet::Starter),
            &mut inventory,
            &mut gear,
        );

        assert_eq!(
            outcome,
            CraftOutcome::Crafted {
                set: GearSet::Starter
            }
        );
        assert_eq!(inventory.crop_count(CropType::Starter), 0);
        assert_eq!(gear.owned.len(), 2);
        assert_eq!(
            gear.equipped(GearSlot::Weapon),
            Some(GearPiece::new(GearSet::Starter, GearSlot::Weapon))
        );
        assert_eq!(
            gear.equipped(GearSlot::Armor),
            Some(GearPiece::new(GearSet::Starter, GearSlot::Armor))
        );
    }

    #[test]
    fn perform_craft_reports_missing_without_spending() {
        let mut inventory = Inventory::default();
        inventory.add_crop(CropType::Starter, 4);
        let mut gear = PlayerGear::default();

        let outcome = perform(
            RowAction::Craft(GearSet::Starter),
            &mut inventory,
            &mut gear,
        );

        match outcome {
            CraftOutcome::Missing { items } => {
                assert_eq!(items, vec![(ItemCost::Crop(CropType::Starter), 6)])
            }
            other => panic!("unexpected outcome {other:?}"),
        }
        assert_eq!(inventory.crop_count(CropType::Starter), 4);
        assert!(gear.owned.is_empty());
    }

    #[test]
    fn perform_craft_refuses_a_set_that_is_already_owned() {
        let mut inventory = Inventory::default();
        inventory.add_crop(CropType::Starter, 30);
        let mut gear = PlayerGear::default();
        perform(
            RowAction::Craft(GearSet::Starter),
            &mut inventory,
            &mut gear,
        );

        let outcome = perform(
            RowAction::Craft(GearSet::Starter),
            &mut inventory,
            &mut gear,
        );

        assert_eq!(
            outcome,
            CraftOutcome::AlreadyOwned {
                set: GearSet::Starter
            }
        );
        assert_eq!(inventory.crop_count(CropType::Starter), 20);
        assert_eq!(gear.owned.len(), 2);
    }

    #[test]
    fn perform_equip_allows_mixing_sets() {
        let mut inventory = Inventory::default();
        inventory.add_crop(CropType::Starter, 10);
        let mut gear = PlayerGear::default();
        perform(
            RowAction::Craft(GearSet::Starter),
            &mut inventory,
            &mut gear,
        );
        gear.own(GearPiece::new(GearSet::Master, GearSlot::Weapon));

        let outcome = perform(
            RowAction::Equip(GearPiece::new(GearSet::Master, GearSlot::Weapon)),
            &mut inventory,
            &mut gear,
        );

        assert_eq!(
            outcome,
            CraftOutcome::Equipped {
                piece: GearPiece::new(GearSet::Master, GearSlot::Weapon)
            }
        );
        assert_eq!(gear.weapon.unwrap().set, GearSet::Master);
        assert_eq!(gear.armor.unwrap().set, GearSet::Starter);
    }

    #[test]
    fn perform_equip_rejects_unowned_piece() {
        let mut inventory = Inventory::default();
        let mut gear = PlayerGear::default();

        let outcome = perform(
            RowAction::Equip(GearPiece::new(GearSet::BossB, GearSlot::Armor)),
            &mut inventory,
            &mut gear,
        );

        assert!(matches!(outcome, CraftOutcome::AlreadyEquipped { .. }));
        assert!(gear.armor.is_none());
    }

    #[test]
    fn outcome_notices_mention_the_reason() {
        assert_eq!(
            CraftOutcome::Crafted {
                set: GearSet::BossA
            }
            .notice(),
            "Crafted Boss A Set"
        );
        assert_eq!(
            CraftOutcome::Equipped {
                piece: GearPiece::new(GearSet::Starter, GearSlot::Weapon)
            }
            .notice(),
            "Equipped Wooden Sword (Starter Set)"
        );
        assert_eq!(
            CraftOutcome::Missing {
                items: vec![
                    (ItemCost::Crop(CropType::CropB), 5),
                    (ItemCost::Material(MaterialType::BossB), 3),
                ]
            }
            .notice(),
            "Missing: Crop B x5, Boss B Material x3"
        );
        assert_eq!(
            CraftOutcome::AlreadyOwned {
                set: GearSet::Master
            }
            .notice(),
            "Master Set already owned"
        );
    }

    #[test]
    fn station_interaction_opens_menu() {
        let mut app = setup_app();
        assert!(!menu(&app).open);
        station_interaction(&mut app);
        assert!(menu(&app).open);
    }

    #[test]
    fn non_crafting_interaction_does_not_open_menu() {
        let mut app = setup_app();
        let entity = app.world_mut().spawn_empty().id();
        app.world_mut().write_message(InteractionEvent {
            entity,
            interaction_type: InteractionType::FarmAction,
        });
        app.update();
        assert!(!menu(&app).open);
    }

    #[test]
    fn interaction_outside_farming_does_not_open_menu() {
        let mut app = setup_app();
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossSelect);
        app.update();
        station_interaction(&mut app);
        assert!(!menu(&app).open);
    }

    #[test]
    fn menu_closes_on_escape_and_on_tab() {
        let mut app = setup_app();
        open_menu(&mut app);
        press(&mut app, KeyCode::Escape);
        assert!(!menu(&app).open);

        open_menu(&mut app);
        press(&mut app, KeyCode::Tab);
        assert!(!menu(&app).open);
    }

    #[test]
    fn menu_closes_when_leaving_farming_phase() {
        let mut app = setup_app();
        open_menu(&mut app);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossFight);
        app.update();
        assert!(!menu(&app).open);
    }

    #[test]
    fn closing_clears_a_pending_notice() {
        let mut app = setup_app();
        grant_starter_crops(&mut app, 10);
        open_menu(&mut app);
        press(&mut app, KeyCode::Enter);
        assert!(!menu(&app).notice.is_empty());
        press(&mut app, KeyCode::Escape);
        assert!(menu(&app).notice.is_empty());
    }

    #[test]
    fn arrow_keys_move_the_selection_within_bounds() {
        let mut app = setup_app();
        open_menu(&mut app);

        press(&mut app, KeyCode::ArrowUp);
        assert_eq!(menu(&app).selected, 0);

        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(menu(&app).selected, 2);

        press(&mut app, KeyCode::ArrowUp);
        assert_eq!(menu(&app).selected, 1);
    }

    #[test]
    fn selection_stops_at_the_last_row() {
        let mut app = setup_app();
        open_menu(&mut app);
        select(&mut app, RECIPE_COUNT - 1);
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(menu(&app).selected, RECIPE_COUNT - 1);
    }

    #[test]
    fn selection_covers_owned_rows_after_crafting() {
        let mut app = setup_app();
        grant_starter_crops(&mut app, 10);
        open_menu(&mut app);
        press(&mut app, KeyCode::Enter);
        assert_eq!(gear(&app).owned.len(), 2);

        for _ in 0..4 {
            press(&mut app, KeyCode::ArrowDown);
        }
        assert_eq!(menu(&app).selected, RECIPE_COUNT);
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(menu(&app).selected, RECIPE_COUNT + 1);
    }

    #[test]
    fn enter_crafts_affordable_recipe_and_emits_messages() {
        let mut app = setup_app();
        grant_starter_crops(&mut app, 10);
        open_menu(&mut app);
        press(&mut app, KeyCode::Enter);

        assert_eq!(inventory(&app).crop_count(CropType::Starter), 0);
        assert_eq!(gear(&app).owned.len(), 2);
        assert_eq!(crafted(&app).len(), 2);
        assert_eq!(equipped(&app), vec![GearSlot::Weapon, GearSlot::Armor]);
        assert_eq!(menu(&app).notice, "Crafted Starter Set");
    }

    #[test]
    fn e_key_also_crafts() {
        let mut app = setup_app();
        grant_starter_crops(&mut app, 10);
        open_menu(&mut app);
        press(&mut app, KeyCode::KeyE);
        assert_eq!(gear(&app).owned.len(), 2);
    }

    #[test]
    fn enter_without_materials_does_nothing_and_reports_missing() {
        let mut app = setup_app();
        open_menu(&mut app);
        press(&mut app, KeyCode::Enter);

        assert!(gear(&app).owned.is_empty());
        assert!(crafted(&app).is_empty());
        assert_eq!(menu(&app).notice, "Missing: Starter Crop x10");
    }

    #[test]
    fn enter_equips_an_owned_piece_row() {
        let mut app = setup_app();
        grant_starter_crops(&mut app, 10);
        open_menu(&mut app);
        press(&mut app, KeyCode::Enter);
        assert_eq!(menu(&app).notice, "Crafted Starter Set");

        app.world_mut()
            .resource_mut::<PlayerGear>()
            .own(GearPiece::new(GearSet::Master, GearSlot::Weapon));
        select(&mut app, OWNED_ROW_OFFSET + 2);
        press(&mut app, KeyCode::Enter);

        assert_eq!(menu(&app).notice, "Equipped Dreamlayer Blade (Master Set)");
        let gear = gear(&app);
        assert_eq!(
            gear.weapon,
            Some(GearPiece::new(GearSet::Master, GearSlot::Weapon))
        );
        assert_eq!(
            gear.armor,
            Some(GearPiece::new(GearSet::Starter, GearSlot::Armor))
        );
    }

    #[test]
    fn enter_on_an_already_equipped_piece_reports_it() {
        let mut app = setup_app();
        grant_starter_crops(&mut app, 10);
        open_menu(&mut app);
        press(&mut app, KeyCode::Enter);

        select(&mut app, OWNED_ROW_OFFSET);
        press(&mut app, KeyCode::Enter);
        assert_eq!(menu(&app).notice, "Wooden Sword is already equipped");

        select(&mut app, OWNED_ROW_OFFSET + 1);
        press(&mut app, KeyCode::Enter);
        assert_eq!(menu(&app).notice, "Cloth Tunic is already equipped");
    }

    #[test]
    fn entering_on_an_owned_set_does_not_craft_twice() {
        let mut app = setup_app();
        grant_starter_crops(&mut app, 30);
        open_menu(&mut app);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);

        assert_eq!(menu(&app).notice, "Starter Set already owned");
        assert_eq!(crafted(&app).len(), 2);
        assert_eq!(inventory(&app).crop_count(CropType::Starter), 20);
    }

    #[test]
    fn crafting_is_ignored_while_menu_is_closed() {
        let mut app = setup_app();
        grant_starter_crops(&mut app, 10);
        press(&mut app, KeyCode::Enter);
        assert!(gear(&app).owned.is_empty());
    }

    #[test]
    fn crafting_is_ignored_outside_farming_phase() {
        let mut app = setup_app();
        grant_starter_crops(&mut app, 10);
        open_menu(&mut app);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossSelect);
        app.update();
        press(&mut app, KeyCode::Enter);
        assert!(gear(&app).owned.is_empty());
    }

    #[test]
    fn clicking_a_recipe_row_crafts_it() {
        let mut app = setup_app();
        grant_starter_crops(&mut app, 10);
        open_menu(&mut app);

        let row = app
            .world_mut()
            .spawn((RecipeRow { index: 0 }, Interaction::None))
            .id();
        app.update();

        *app.world_mut().get_mut::<Interaction>(row).unwrap() = Interaction::Pressed;
        app.update();

        assert_eq!(gear(&app).owned.len(), 2);
        assert_eq!(menu(&app).selected, 0);
        assert_eq!(menu(&app).notice, "Crafted Starter Set");
    }

    #[test]
    fn clicking_an_owned_row_equips_that_piece() {
        let mut app = setup_app();
        grant_starter_crops(&mut app, 10);
        open_menu(&mut app);
        press(&mut app, KeyCode::Enter);

        let row = app
            .world_mut()
            .spawn((
                RecipeRow {
                    index: OWNED_ROW_OFFSET,
                },
                Interaction::None,
            ))
            .id();
        app.update();
        *app.world_mut().get_mut::<Interaction>(row).unwrap() = Interaction::Pressed;
        app.update();

        assert_eq!(menu(&app).notice, "Wooden Sword is already equipped");
        assert_eq!(
            gear(&app).equipped(GearSlot::Weapon),
            Some(GearPiece::new(GearSet::Starter, GearSlot::Weapon))
        );
    }

    #[test]
    fn clicking_while_menu_is_closed_does_nothing() {
        let mut app = setup_app();
        grant_starter_crops(&mut app, 10);

        let row = app
            .world_mut()
            .spawn((RecipeRow { index: 0 }, Interaction::None))
            .id();
        app.update();
        *app.world_mut().get_mut::<Interaction>(row).unwrap() = Interaction::Pressed;
        app.update();

        assert!(gear(&app).owned.is_empty());
    }
}
