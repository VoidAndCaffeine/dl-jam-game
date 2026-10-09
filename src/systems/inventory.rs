use crate::components::gear::{GearPiece, MaterialType};
use crate::components::pot::CropType;
use crate::events::{GearCrafted, GearEquipped};
use crate::resources::boss_select::BossSelectMenu;
use crate::resources::crafting_menu::CraftingMenu;
use crate::resources::crop_select::CropSelectMenu;
use crate::resources::farm::CropUnlocks;
use crate::resources::inventory::Inventory;
use crate::resources::inventory_panel::InventoryPanel;
use crate::resources::run_data::PlayerGear;
use crate::states::{GameState, Phase};
use crate::systems::crafting::{CraftOutcome, RowAction, apply};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::state::state::State;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum InventoryPanelSet {
    Panel,
}

#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct InventorySlot {
    pub index: usize,
}

/// One line in the inventory panel. Locked crops and materials the player holds
/// none of are left out entirely so the panel only shows reachable items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryRow {
    Crop(CropType),
    Material(MaterialType),
    Gear(GearPiece),
}

impl InventoryRow {
    pub fn label(&self) -> String {
        match self {
            InventoryRow::Crop(crop) => crop.label().to_string(),
            InventoryRow::Material(material) => material.label().to_string(),
            InventoryRow::Gear(piece) => piece.describe(),
        }
    }

    /// Gear rows have no stack count, so they report `None`.
    pub fn count(&self, inventory: &Inventory) -> Option<u32> {
        match self {
            InventoryRow::Crop(crop) => Some(inventory.crop_count(*crop)),
            InventoryRow::Material(material) => Some(inventory.material_count(*material)),
            InventoryRow::Gear(_) => None,
        }
    }
}

pub fn inventory_rows(
    inventory: &Inventory,
    gear: &PlayerGear,
    unlocks: &CropUnlocks,
) -> Vec<InventoryRow> {
    let mut rows: Vec<InventoryRow> = CropType::ALL
        .iter()
        .filter(|crop| unlocks.is_unlocked(**crop) || inventory.crop_count(**crop) > 0)
        .map(|crop| InventoryRow::Crop(*crop))
        .collect();
    rows.extend(
        MaterialType::ALL
            .iter()
            .filter(|material| inventory.material_count(**material) > 0)
            .map(|material| InventoryRow::Material(*material)),
    );
    rows.extend(gear.owned.iter().map(|piece| InventoryRow::Gear(*piece)));
    rows
}

pub fn row_action(row: usize, rows: &[InventoryRow]) -> Option<RowAction> {
    match rows.get(row)? {
        InventoryRow::Gear(piece) => Some(RowAction::Equip(*piece)),
        _ => None,
    }
}

/// The resources both equip entry points share.
#[derive(bevy::ecs::system::SystemParam)]
pub struct EquipWork<'w> {
    pub panel: ResMut<'w, InventoryPanel>,
    pub inventory: ResMut<'w, Inventory>,
    pub gear: ResMut<'w, PlayerGear>,
    pub unlocks: Res<'w, CropUnlocks>,
    pub crafted: MessageWriter<'w, GearCrafted>,
    pub equipped: MessageWriter<'w, GearEquipped>,
    pub game_state: Res<'w, State<GameState>>,
}

impl EquipWork<'_> {
    fn active(&self) -> bool {
        panel_active(&self.game_state, &self.panel)
    }

    fn rows(&self) -> Vec<InventoryRow> {
        inventory_rows(&self.inventory, &self.gear, &self.unlocks)
    }

    fn apply(&mut self, action: RowAction) -> CraftOutcome {
        apply(
            action,
            &mut self.inventory,
            &mut self.gear,
            &mut self.crafted,
            &mut self.equipped,
        )
    }
}

fn panel_active(game_state: &State<GameState>, panel: &InventoryPanel) -> bool {
    matches!(game_state.get(), GameState::Playing) && panel.open
}

/// The inventory is reachable from any phase, so it only closes on its own input
/// or on leaving `GameState::Playing`.
pub fn toggle_inventory_panel(
    keys: Res<ButtonInput<KeyCode>>,
    mut panel: ResMut<InventoryPanel>,
    mut menu: ResMut<CraftingMenu>,
    pause: Option<Res<crate::resources::pause::PauseMenu>>,
    phase: Phase,
) {
    if !phase.blocks_world() {
        return;
    }
    // The pause overlay owns input while it is up.
    if pause.as_ref().is_some_and(|pause| pause.open) {
        return;
    }
    if keys.just_pressed(KeyCode::KeyI) {
        panel.toggle_panel();
    }
    if panel.open {
        menu.close_menu();
    }
}

pub fn close_inventory_on_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut panel: ResMut<InventoryPanel>,
    phase: Phase,
) {
    if !panel.open || !phase.blocks_world() {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        panel.close_panel();
    }
}

pub fn close_inventory_outside_playing(mut panel: ResMut<InventoryPanel>, phase: Phase) {
    if panel.open && !phase.blocks_world() {
        panel.close_panel();
    }
}

/// Belt-and-braces guard: the crafting menu can open from a station interaction in
/// the same frame the inventory opens, and only one panel may be open at a time.
/// The boss select menu is a panel too, so opening either other panel backs out of
/// it (and the boss phase recovers to farming). The crop picker is transient, so
/// any other panel simply closes it.
pub fn enforce_single_open_panel(
    mut menu: ResMut<CraftingMenu>,
    panel: ResMut<InventoryPanel>,
    mut boss: ResMut<BossSelectMenu>,
    mut crop_select: ResMut<CropSelectMenu>,
) {
    if panel.open && menu.open {
        menu.close_menu();
    }
    if boss.open && (menu.open || panel.open) {
        boss.close_menu();
    }
    if crop_select.open && (menu.open || panel.open || boss.open) {
        crop_select.close_menu();
    }
}

pub fn move_panel_selection(
    keys: Res<ButtonInput<KeyCode>>,
    mut panel: ResMut<InventoryPanel>,
    inventory: Res<Inventory>,
    gear: Res<PlayerGear>,
    unlocks: Res<CropUnlocks>,
    phase: Phase,
) {
    if !panel.open || !phase.blocks_world() {
        return;
    }
    let rows = inventory_rows(&inventory, &gear, &unlocks);
    let last_row = rows.len().saturating_sub(1);
    let next = if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) {
        Some(panel.selected + 1)
    } else if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) {
        Some(panel.selected.saturating_sub(1))
    } else {
        None
    };
    let Some(next) = next else {
        return;
    };
    panel.selected = next.min(last_row);
}

pub fn equip_selected_row(keys: Res<ButtonInput<KeyCode>>, mut work: EquipWork) {
    if !work.active() {
        return;
    }
    if !keys.just_pressed(KeyCode::Enter) && !keys.just_pressed(KeyCode::KeyE) {
        return;
    }
    let selected = work.panel.selected;
    let Some(action) = row_action(selected, &work.rows()) else {
        return;
    };
    let outcome = work.apply(action);
    work.panel.set_notice(outcome.notice());
}

pub fn equip_row_on_click(
    rows: Query<(Entity, &InventorySlot), Changed<Interaction>>,
    interactions: Query<&Interaction>,
    mut work: EquipWork,
) {
    if !work.active() {
        return;
    }
    for (entity, slot) in rows.iter() {
        let Ok(interaction) = interactions.get(entity) else {
            continue;
        };
        if *interaction != Interaction::Pressed {
            continue;
        }
        work.panel.selected = slot.index;
        let index = slot.index;
        let Some(action) = row_action(index, &work.rows()) else {
            continue;
        };
        let outcome = work.apply(action);
        work.panel.set_notice(outcome.notice());
    }
}

pub fn close_inventory_panel(mut panel: ResMut<InventoryPanel>) {
    panel.close_panel();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::gear::{GearSet, GearSlot};
    use crate::events::InteractionEvent;
    use crate::states::DayPhase;
    use crate::systems::crafting::CraftingMenuSet;
    use bevy::ecs::message::MessageReader;
    use bevy::state::app::StatesPlugin;
    use bevy::transform::TransformPlugin;

    #[derive(Resource, Default)]
    struct CapturedEquipped(Vec<GearSlot>);

    #[derive(Resource, Default)]
    struct CapturedCrafted(Vec<GearPiece>);

    fn capture_equipped(
        mut captured: ResMut<CapturedEquipped>,
        mut reader: MessageReader<GearEquipped>,
    ) {
        for event in reader.read() {
            captured.0.push(event.0);
        }
    }

    fn capture_crafted(
        mut captured: ResMut<CapturedCrafted>,
        mut reader: MessageReader<GearCrafted>,
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
            .init_resource::<InventoryPanel>()
            .init_resource::<BossSelectMenu>()
            .init_resource::<CropSelectMenu>()
            .init_resource::<Inventory>()
            .init_resource::<PlayerGear>()
            .init_resource::<CropUnlocks>()
            .init_resource::<CapturedEquipped>()
            .init_resource::<CapturedCrafted>()
            .add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_message::<InteractionEvent>()
            .add_message::<GearCrafted>()
            .add_message::<GearEquipped>()
            .add_systems(
                Update,
                (
                    toggle_inventory_panel,
                    close_inventory_on_input,
                    close_inventory_outside_playing,
                    move_panel_selection,
                    equip_selected_row,
                    equip_row_on_click,
                )
                    .chain()
                    .in_set(InventoryPanelSet::Panel),
            )
            .add_systems(
                Update,
                (
                    enforce_single_open_panel
                        .after(InventoryPanelSet::Panel)
                        .after(CraftingMenuSet::Menu),
                    capture_equipped.after(InventoryPanelSet::Panel),
                    capture_crafted.after(InventoryPanelSet::Panel),
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

    fn open_panel(app: &mut App) {
        app.world_mut()
            .resource_mut::<InventoryPanel>()
            .open_panel();
    }

    fn select(app: &mut App, row: usize) {
        app.world_mut().resource_mut::<InventoryPanel>().selected = row;
    }

    fn panel(app: &App) -> &InventoryPanel {
        app.world().resource::<InventoryPanel>()
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

    fn owned(app: &mut App, set: GearSet, slot: GearSlot) {
        app.world_mut()
            .resource_mut::<PlayerGear>()
            .own(GearPiece::new(set, slot));
    }

    fn grant_materials(app: &mut App, material: MaterialType, amount: u32) {
        app.world_mut()
            .resource_mut::<Inventory>()
            .add_material(material, amount);
    }

    fn equipped(app: &App) -> Vec<GearSlot> {
        app.world().resource::<CapturedEquipped>().0.clone()
    }

    fn crafted(app: &App) -> Vec<GearPiece> {
        app.world().resource::<CapturedCrafted>().0.clone()
    }

    fn rows(app: &App) -> Vec<InventoryRow> {
        inventory_rows(
            app.world().resource::<Inventory>(),
            app.world().resource::<PlayerGear>(),
            app.world().resource::<CropUnlocks>(),
        )
    }

    fn select_gear_row(app: &mut App) {
        let index = first_gear_row(&rows(app));
        select(app, index);
    }

    fn first_gear_row(rows: &[InventoryRow]) -> usize {
        rows.iter()
            .position(|row| matches!(row, InventoryRow::Gear(_)))
            .expect("a gear row exists")
    }

    #[test]
    fn locked_crops_are_hidden() {
        let app = setup_app();
        let rows = rows(&app);
        assert_eq!(rows, vec![InventoryRow::Crop(CropType::Starter)]);
    }

    #[test]
    fn unlocking_a_crop_adds_its_row() {
        let mut app = setup_app();
        app.world_mut()
            .resource_mut::<CropUnlocks>()
            .unlock_crop_a();

        let rows = rows(&app);
        assert_eq!(
            rows,
            vec![
                InventoryRow::Crop(CropType::Starter),
                InventoryRow::Crop(CropType::CropA),
            ]
        );
    }

    #[test]
    fn a_crop_with_stock_shows_even_while_locked() {
        let mut app = setup_app();
        app.world_mut()
            .resource_mut::<Inventory>()
            .add_crop(CropType::CropB, 2);

        assert!(rows(&app).contains(&InventoryRow::Crop(CropType::CropB)));
    }

    #[test]
    fn materials_with_no_stock_are_hidden() {
        let app = setup_app();
        assert!(
            !rows(&app)
                .iter()
                .any(|row| matches!(row, InventoryRow::Material(_)))
        );
    }

    #[test]
    fn materials_with_stock_are_listed() {
        let mut app = setup_app();
        app.world_mut()
            .resource_mut::<Inventory>()
            .add_material(MaterialType::BossB2, 4);

        assert!(rows(&app).contains(&InventoryRow::Material(MaterialType::BossB2)));
        assert!(!rows(&app).contains(&InventoryRow::Material(MaterialType::BossA1)));
    }

    #[test]
    fn owned_gear_rows_come_last_in_owned_order() {
        let mut app = setup_app();
        owned(&mut app, GearSet::Starter, GearSlot::Armor);
        owned(&mut app, GearSet::Master, GearSlot::Weapon);

        let rows = rows(&app);
        assert_eq!(
            rows,
            vec![
                InventoryRow::Crop(CropType::Starter),
                InventoryRow::Gear(GearPiece::new(GearSet::Starter, GearSlot::Armor)),
                InventoryRow::Gear(GearPiece::new(GearSet::Master, GearSlot::Weapon)),
            ]
        );
    }

    #[test]
    fn row_labels_cover_every_kind() {
        assert_eq!(
            InventoryRow::Crop(CropType::Starter).label(),
            "Starter Crop"
        );
        assert_eq!(
            InventoryRow::Material(MaterialType::BossA1).label(),
            "Boss A Material 1"
        );
        assert_eq!(
            InventoryRow::Gear(GearPiece::new(GearSet::Starter, GearSlot::Weapon)).label(),
            "Starter Spearblade (Starter Set)"
        );
    }

    #[test]
    fn row_counts_read_stock_and_gear_rows_have_none() {
        let mut app = setup_app();
        app.world_mut()
            .resource_mut::<Inventory>()
            .add_crop(CropType::Starter, 6);
        app.world_mut()
            .resource_mut::<Inventory>()
            .add_material(MaterialType::BossA1, 2);
        owned(&mut app, GearSet::Starter, GearSlot::Weapon);

        assert_eq!(
            InventoryRow::Crop(CropType::Starter).count(inventory(&app)),
            Some(6)
        );
        assert_eq!(
            InventoryRow::Material(MaterialType::BossA1).count(inventory(&app)),
            Some(2)
        );
        assert_eq!(
            InventoryRow::Gear(GearPiece::new(GearSet::Starter, GearSlot::Weapon))
                .count(inventory(&app)),
            None
        );
    }

    #[test]
    fn row_action_only_maps_gear_rows() {
        let mut app = setup_app();
        owned(&mut app, GearSet::Master, GearSlot::Weapon);
        let piece = GearPiece::new(GearSet::Master, GearSlot::Weapon);
        let rows = rows(&app);

        assert_eq!(
            row_action(first_gear_row(&rows), &rows),
            Some(RowAction::Equip(piece))
        );
        assert_eq!(row_action(0, &rows), None);
    }

    #[test]
    fn row_action_out_of_range_is_none() {
        assert_eq!(row_action(0, &[]), None);
        assert_eq!(
            row_action(3, &[InventoryRow::Crop(CropType::Starter)]),
            None
        );
    }

    #[test]
    fn i_key_opens_the_panel() {
        let mut app = setup_app();
        assert!(!panel(&app).open);
        press(&mut app, KeyCode::KeyI);
        assert!(panel(&app).open);
    }

    #[test]
    fn i_key_closes_the_panel_again() {
        let mut app = setup_app();
        press(&mut app, KeyCode::KeyI);
        press(&mut app, KeyCode::KeyI);
        assert!(!panel(&app).open);
    }

    #[test]
    fn the_panel_opens_in_every_day_phase() {
        for phase in [
            DayPhase::Farming,
            DayPhase::BossSelect,
            DayPhase::BossFight,
            DayPhase::Result,
        ] {
            let mut app = setup_app();
            app.world_mut()
                .resource_mut::<NextState<DayPhase>>()
                .set(phase.clone());
            app.update();

            press(&mut app, KeyCode::KeyI);
            assert!(panel(&app).open, "panel must open during {phase:?}");
        }
    }

    #[test]
    fn the_panel_stays_open_across_a_day_phase_change() {
        let mut app = setup_app();
        press(&mut app, KeyCode::KeyI);
        assert!(panel(&app).open);

        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossFight);
        app.update();

        assert!(panel(&app).open);
    }

    #[test]
    fn i_is_ignored_outside_playing() {
        let mut app = setup_app();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();

        press(&mut app, KeyCode::KeyI);
        assert!(!panel(&app).open);
    }

    #[test]
    fn escape_closes_the_panel() {
        let mut app = setup_app();
        open_panel(&mut app);
        press(&mut app, KeyCode::Escape);
        assert!(!panel(&app).open);
    }

    #[test]
    fn tab_stays_with_the_crafting_menu() {
        let mut app = setup_app();
        press(&mut app, KeyCode::Tab);
        assert!(!panel(&app).open);

        open_panel(&mut app);
        app.world_mut().resource_mut::<CraftingMenu>().open_menu();
        press(&mut app, KeyCode::Tab);

        assert!(panel(&app).open, "tab must not close the inventory");
        assert!(!menu(&app).open, "tab must still close crafting");
    }

    #[test]
    fn opening_the_inventory_closes_the_crafting_menu() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<CraftingMenu>().open_menu();
        press(&mut app, KeyCode::KeyI);

        assert!(panel(&app).open);
        assert!(!menu(&app).open);
    }

    #[test]
    fn the_guard_closes_the_crafting_menu_while_the_inventory_is_open() {
        let mut app = setup_app();
        open_panel(&mut app);
        app.update();
        app.world_mut().resource_mut::<CraftingMenu>().open_menu();
        app.update();

        assert!(panel(&app).open);
        assert!(!menu(&app).open);
    }

    #[test]
    fn the_guard_leaves_the_crafting_menu_alone_while_the_inventory_is_closed() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<CraftingMenu>().open_menu();
        app.update();

        assert!(menu(&app).open);
    }

    #[test]
    fn opening_the_inventory_closes_the_crop_picker() {
        let mut app = setup_app();
        app.world_mut()
            .resource_mut::<CropSelectMenu>()
            .open_menu(0);
        open_panel(&mut app);
        app.update();

        assert!(panel(&app).open);
        assert!(!app.world().resource::<CropSelectMenu>().open);
    }

    #[test]
    fn the_guard_closes_the_crop_picker_while_the_crafting_menu_is_open() {
        let mut app = setup_app();
        app.world_mut()
            .resource_mut::<CropSelectMenu>()
            .open_menu(0);
        app.world_mut().resource_mut::<CraftingMenu>().open_menu();
        app.update();

        assert!(!app.world().resource::<CropSelectMenu>().open);
    }

    #[test]
    fn both_panels_are_never_open_at_once() {
        let mut app = setup_app();
        press(&mut app, KeyCode::KeyI);
        app.world_mut().resource_mut::<CraftingMenu>().open_menu();
        app.update();

        assert!(panel(&app).open);
        assert!(!menu(&app).open);
    }

    #[test]
    fn the_panel_closes_when_leaving_playing() {
        let mut app = setup_app();
        open_panel(&mut app);
        assert!(panel(&app).open);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();

        assert!(!panel(&app).open);
    }

    #[test]
    fn arrow_keys_move_the_selection_within_bounds() {
        let mut app = setup_app();
        grant_materials(&mut app, MaterialType::BossA1, 1);
        grant_materials(&mut app, MaterialType::BossB1, 1);
        open_panel(&mut app);

        press(&mut app, KeyCode::ArrowUp);
        assert_eq!(panel(&app).selected, 0);

        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(panel(&app).selected, 1);

        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(panel(&app).selected, 2);

        press(&mut app, KeyCode::ArrowUp);
        assert_eq!(panel(&app).selected, 1);
    }

    #[test]
    fn the_selection_stops_at_the_last_row() {
        let mut app = setup_app();
        grant_materials(&mut app, MaterialType::BossA1, 1);
        open_panel(&mut app);
        select(&mut app, 1);

        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(panel(&app).selected, 1, "two rows is the last index");
    }

    #[test]
    fn the_selection_covers_gear_rows() {
        let mut app = setup_app();
        owned(&mut app, GearSet::Starter, GearSlot::Weapon);
        owned(&mut app, GearSet::Starter, GearSlot::Armor);
        open_panel(&mut app);

        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(panel(&app).selected, 1);
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(panel(&app).selected, 2);
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(panel(&app).selected, 2);
    }

    #[test]
    fn enter_equips_the_selected_gear_row_and_emits_a_message() {
        let mut app = setup_app();
        owned(&mut app, GearSet::Master, GearSlot::Weapon);
        open_panel(&mut app);
        select_gear_row(&mut app);
        press(&mut app, KeyCode::Enter);

        assert_eq!(
            gear(&app).equipped(GearSlot::Weapon),
            Some(GearPiece::new(GearSet::Master, GearSlot::Weapon))
        );
        assert_eq!(equipped(&app), vec![GearSlot::Weapon]);
        assert_eq!(
            panel(&app).notice,
            "Equipped Dreaming Spearblade (Master Set)"
        );
    }

    #[test]
    fn e_key_also_equips() {
        let mut app = setup_app();
        owned(&mut app, GearSet::Starter, GearSlot::Weapon);
        open_panel(&mut app);
        select_gear_row(&mut app);
        press(&mut app, KeyCode::KeyE);

        assert_eq!(
            gear(&app).equipped(GearSlot::Weapon),
            Some(GearPiece::new(GearSet::Starter, GearSlot::Weapon))
        );
    }

    #[test]
    fn enter_on_a_crop_row_does_nothing() {
        let mut app = setup_app();
        open_panel(&mut app);
        select(&mut app, 0);
        press(&mut app, KeyCode::Enter);

        assert!(equipped(&app).is_empty());
        assert!(crafted(&app).is_empty());
        assert!(panel(&app).notice.is_empty());
    }

    #[test]
    fn enter_on_an_already_equipped_piece_reports_it() {
        let mut app = setup_app();
        let piece = GearPiece::new(GearSet::Starter, GearSlot::Armor);
        owned(&mut app, GearSet::Starter, GearSlot::Armor);
        {
            let mut gear = app.world_mut().resource_mut::<PlayerGear>();
            gear.own(piece);
            gear.equip(&piece);
        }
        open_panel(&mut app);
        select_gear_row(&mut app);
        press(&mut app, KeyCode::Enter);

        assert_eq!(panel(&app).notice, "Cloth Tunic is already equipped");
        assert!(equipped(&app).is_empty());
    }

    #[test]
    fn equipping_is_ignored_while_the_panel_is_closed() {
        let mut app = setup_app();
        owned(&mut app, GearSet::Starter, GearSlot::Weapon);
        select_gear_row(&mut app);
        press(&mut app, KeyCode::Enter);

        assert!(gear(&app).weapon.is_none());
        assert!(equipped(&app).is_empty());
    }

    #[test]
    fn equipping_is_ignored_outside_playing() {
        let mut app = setup_app();
        owned(&mut app, GearSet::Starter, GearSlot::Weapon);
        open_panel(&mut app);
        select_gear_row(&mut app);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();
        press(&mut app, KeyCode::Enter);

        assert!(gear(&app).weapon.is_none());
    }

    #[test]
    fn equipping_works_during_the_boss_fight() {
        let mut app = setup_app();
        owned(&mut app, GearSet::BossA, GearSlot::Weapon);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossFight);
        app.update();
        open_panel(&mut app);
        select_gear_row(&mut app);
        press(&mut app, KeyCode::Enter);

        assert_eq!(
            gear(&app).equipped(GearSlot::Weapon),
            Some(GearPiece::new(GearSet::BossA, GearSlot::Weapon))
        );
    }

    #[test]
    fn clicking_a_gear_row_equips_it() {
        let mut app = setup_app();
        owned(&mut app, GearSet::Starter, GearSlot::Weapon);
        open_panel(&mut app);
        let index = first_gear_row(&rows(&app));

        let slot = app
            .world_mut()
            .spawn((InventorySlot { index }, Interaction::None))
            .id();
        app.update();
        *app.world_mut().get_mut::<Interaction>(slot).unwrap() = Interaction::Pressed;
        app.update();

        assert_eq!(panel(&app).selected, index);
        assert_eq!(
            gear(&app).equipped(GearSlot::Weapon),
            Some(GearPiece::new(GearSet::Starter, GearSlot::Weapon))
        );
        assert_eq!(
            panel(&app).notice,
            "Equipped Starter Spearblade (Starter Set)"
        );
    }

    #[test]
    fn clicking_a_crop_row_does_nothing() {
        let mut app = setup_app();
        open_panel(&mut app);

        let slot = app
            .world_mut()
            .spawn((InventorySlot { index: 0 }, Interaction::None))
            .id();
        app.update();
        *app.world_mut().get_mut::<Interaction>(slot).unwrap() = Interaction::Pressed;
        app.update();

        assert!(panel(&app).notice.is_empty());
        assert!(crafted(&app).is_empty());
    }

    #[test]
    fn clicking_while_the_panel_is_closed_does_nothing() {
        let mut app = setup_app();
        owned(&mut app, GearSet::Starter, GearSlot::Weapon);
        let index = first_gear_row(&rows(&app));

        let slot = app
            .world_mut()
            .spawn((InventorySlot { index }, Interaction::None))
            .id();
        app.update();
        *app.world_mut().get_mut::<Interaction>(slot).unwrap() = Interaction::Pressed;
        app.update();

        assert!(gear(&app).weapon.is_none());
    }

    #[test]
    fn closing_the_panel_clears_a_pending_notice() {
        let mut app = setup_app();
        owned(&mut app, GearSet::Starter, GearSlot::Weapon);
        open_panel(&mut app);
        select_gear_row(&mut app);
        press(&mut app, KeyCode::Enter);
        assert!(!panel(&app).notice.is_empty());

        press(&mut app, KeyCode::KeyI);
        assert!(panel(&app).notice.is_empty());
        assert_eq!(panel(&app).selected, 0);
    }

    #[test]
    fn equipped_gear_from_the_inventory_never_crafts_new_pieces() {
        let mut app = setup_app();
        owned(&mut app, GearSet::Starter, GearSlot::Weapon);
        open_panel(&mut app);
        select_gear_row(&mut app);
        press(&mut app, KeyCode::Enter);

        assert!(crafted(&app).is_empty());
        assert_eq!(gear(&app).owned.len(), 1);
    }
}
