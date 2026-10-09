//! The in-game menus: the forge (gear), the inventory, boss select and the
//! planting picker.
//!
//! All four are drawn from the shared widgets in [`crate::plugins::ui_theme`],
//! so a button is a styled row that can be reskinned with PNGs and a pane is a
//! flexible column. Selection, clicks and actions still live in the matching
//! `systems/*` module; this file only lays out and refreshes the visuals.

use crate::components::boss::BossId;
use crate::components::gear::{GearPiece, GearSlot, RECIPE_COUNT, recipe_for_piece};
use crate::components::player_sprite::{Facing8, PlayerAnimState, PlayerLook};
use crate::components::pot::CropType;
use crate::plugins::ui_theme::{self as theme, AnimatedPreview, MenuArt};
use crate::resources::boss_progress::BossProgress;
use crate::resources::boss_select::BossSelectMenu;
use crate::resources::crafting_menu::CraftingMenu;
use crate::resources::crop_select::CropSelectMenu;
use crate::resources::farm::CropUnlocks;
use crate::resources::inventory::Inventory;
use crate::resources::inventory_panel::InventoryPanel;
use crate::resources::menu_text;
use crate::resources::player_sprite::{PlayerSpriteAssets, SpriteKey};
use crate::resources::run_data::PlayerGear;
use crate::states::GameState;
use crate::systems::boss_select::{BossOption, BossSelectSet};
use crate::systems::crafting::{CraftingMenuSet, RecipeRow, RowAction, row_action};
use crate::systems::crop_select::{CropOption, CropSelectSet};
use crate::systems::inventory::{
    InventoryPanelSet, InventoryRow, InventorySlot, close_inventory_on_input,
    close_inventory_outside_playing, close_inventory_panel, enforce_single_open_panel,
    equip_row_on_click, equip_selected_row, inventory_rows, move_panel_selection,
    toggle_inventory_panel,
};
use bevy::prelude::*;
#[allow(unused_imports)]
use bevy::ui::FocusPolicy;

// --- Palette (aliased from the shared theme) ---------------------------------

const RECIPE_NAME_WIDTH: f32 = 200.0;
const OWNED_NAME_WIDTH: f32 = 200.0;
const ITEM_NAME_WIDTH: f32 = 210.0;

const PANEL_BG: Color = theme::PANEL_BG;
const PANEL_BORDER: Color = theme::PANEL_BORDER;
const TEXT_PRIMARY: Color = theme::TEXT_PRIMARY;
const TEXT_DIM: Color = theme::TEXT_DIM;
const TEXT_CRAFTABLE: Color = theme::TEXT_CRAFTABLE;
const TEXT_BLOCKED: Color = theme::TEXT_BLOCKED;
#[allow(dead_code)]
const ROW_BG: Color = theme::ROW_BG;
#[allow(dead_code)]
const ROW_BG_HOVER: Color = theme::ROW_BG_HOVER;
const ROW_BORDER: Color = theme::ROW_BORDER;
#[allow(dead_code)]
const ROW_BORDER_SELECTED: Color = theme::ROW_BORDER_SELECTED;

// The boss concept art paths. The plant seedlings live under `sprite_packs`
// as 5x5 sheets (same layout as the effects), animated in the planting pane.
const BOSS_A_CONCEPT: &str = "refrence_images/boss_a_excavator_concept.png";
const BOSS_B_CONCEPT: &str = "refrence_images/boss_b_quicksilver_concept.png";
const PLANT_CINDER_CAP: &str = "sprite_packs/Plants/Cinder Cap Seedling/spritesheet.png";
const PLANT_QUICKSILVER_REED: &str =
    "sprite_packs/Plants/Quicksilver Reed Seedling/spritesheet.png";
const PLANT_TAILINGS_POTATO: &str = "sprite_packs/Plants/Tailings Potato Seedling/spritesheet.png";

/// The side the animated previews are drawn at inside their panes.
const PREVIEW_SIZE: f32 = 200.0;

// --- Markers -----------------------------------------------------------------

#[derive(Component, Reflect, Debug, Default)]
pub struct CraftingMenuRoot;

#[derive(Component, Reflect, Debug, Default)]
pub struct InventoryRoot;

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuText {
    Line(MenuLine),
    RowName(usize),
    RowStatus(usize),
}

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuLine {
    Inventory,
    Equipped,
    Notice,
}

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryText {
    Line(InventoryLine),
    RowName(usize),
    RowDetail(usize),
}

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryLine {
    Equipped,
    Notice,
}

#[derive(Component, Reflect, Debug, Default)]
pub struct BossSelectRoot;

#[derive(Component, Reflect, Debug, Default)]
pub struct BossConfirmRoot;

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossSelectText {
    Title,
    OptionName(usize),
    OptionStatus(usize),
    ConfirmPrompt,
}

#[derive(Component, Reflect, Debug, Default)]
pub struct CropSelectRoot;

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropSelectText {
    Title,
    RowName(usize),
    RowStatus(usize),
    RowYield(usize),
    Hint,
}

/// The animated gear showcase on the left of the forge.
#[derive(Component, Reflect, Debug, Default)]
pub struct GearPreview;

/// The description and the material list on the right of the forge.
#[derive(Component, Reflect, Debug, Default)]
pub struct GearDescription;

#[derive(Component, Reflect, Debug, Default)]
pub struct GearMaterials;

/// One of the two concept-art slots in the boss pane. A single boss shows its
/// own slot; the dual boss shows both halves at once.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct BossConceptSlot {
    pub boss: BossId,
}

/// The concept-art pane on the left of boss select.
#[derive(Component, Reflect, Debug, Default)]
pub struct BossConcept;

/// The sapling on the left of the planting picker.
#[derive(Component, Reflect, Debug, Default)]
pub struct PlantPreview;

/// A camera for the full-screen menus that have no world of their own
/// (`MainMenu`, `Victory`). `Playing` spawns its own camera in `spawn_player`,
/// and the boot loading screen uses a temporary one, so without this the title
/// and victory screens would render with no camera at all.
#[derive(Component, Reflect, Debug, Default)]
pub struct MenuCamera;

fn ensure_menu_camera(mut commands: Commands, cameras: Query<(), With<Camera2d>>) {
    if cameras.is_empty() {
        commands.spawn((Name::new("Menu Camera"), MenuCamera, Camera2d));
    }
}

fn despawn_menu_camera(mut commands: Commands, cameras: Query<Entity, With<MenuCamera>>) {
    for entity in cameras.iter() {
        commands.entity(entity).despawn();
    }
}

// --- Shared helpers ----------------------------------------------------------

/// Status line for a crop row: locked rows explain which boss unlocks them.
fn crop_status(crop: CropType, unlocks: &CropUnlocks) -> (String, Color) {
    if !unlocks.is_unlocked(crop) {
        let hint = match crop {
            CropType::Starter => "Locked",
            CropType::CropA => "Beat Boss A",
            CropType::CropB => "Beat Boss B",
        };
        return (format!("LOCKED  -  {hint}"), TEXT_BLOCKED);
    }
    (format!("{} days", crop.growth_days()), TEXT_PRIMARY)
}

fn crop_name_color(crop: CropType, unlocks: &CropUnlocks) -> Color {
    if unlocks.is_unlocked(crop) {
        TEXT_PRIMARY
    } else {
        TEXT_DIM
    }
}

/// Status line for a boss row: locked rows explain how to open them.
fn boss_status(id: BossId, progress: &BossProgress) -> (String, Color) {
    if !progress.is_unlocked(id) {
        return ("LOCKED  -  Beat Boss A + Boss B".to_string(), TEXT_BLOCKED);
    }
    if progress.is_beaten(id) {
        ("BEATEN".to_string(), TEXT_CRAFTABLE)
    } else {
        ("AVAILABLE".to_string(), TEXT_PRIMARY)
    }
}

fn boss_name_color(id: BossId, progress: &BossProgress) -> Color {
    if progress.is_unlocked(id) {
        TEXT_PRIMARY
    } else {
        TEXT_DIM
    }
}

fn boss_concept_path(id: BossId) -> &'static str {
    match id {
        BossId::BossA => BOSS_A_CONCEPT,
        BossId::BossB => BOSS_B_CONCEPT,
        // The dual fight shows both halves; this is only the fallback.
        BossId::Dual => BOSS_A_CONCEPT,
    }
}

fn plant_path(crop: CropType) -> &'static str {
    match crop {
        CropType::Starter => PLANT_QUICKSILVER_REED,
        CropType::CropA => PLANT_CINDER_CAP,
        CropType::CropB => PLANT_TAILINGS_POTATO,
    }
}

fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

fn default_visuals(art: &mut MenuArt, server: Option<&AssetServer>) -> theme::MenuButtonVisuals {
    art.shared_visuals(server)
}

fn boss_skin(id: BossId) -> &'static str {
    match id {
        BossId::BossA => "boss_a",
        BossId::BossB => "boss_b",
        BossId::Dual => "dual_boss",
    }
}

fn crop_skin(crop: CropType) -> &'static str {
    match crop {
        CropType::Starter => "starter_crop",
        CropType::CropA => "crop_a",
        CropType::CropB => "crop_b",
    }
}

fn selected_piece(menu: &CraftingMenu, gear: &PlayerGear) -> Option<GearPiece> {
    match row_action(menu.selected, &gear.owned)? {
        RowAction::Craft(piece) | RowAction::Equip(piece) => Some(piece),
    }
}

fn row_title(index: usize, gear: &PlayerGear) -> String {
    match row_action(index, &gear.owned) {
        Some(RowAction::Craft(piece)) => piece.name().to_string(),
        Some(RowAction::Equip(piece)) => piece.name().to_string(),
        None => String::new(),
    }
}

fn recipe_status(piece: GearPiece, inventory: &Inventory, gear: &PlayerGear) -> (String, Color) {
    let recipe = recipe_for_piece(piece);
    let have = recipe
        .cost
        .iter()
        .map(|(item, required)| format!("{} {}/{}", item.label(), inventory.count(item), required))
        .collect::<Vec<String>>()
        .join(", ");
    if gear.owns(&piece) {
        (format!("{have}  -  Owned"), TEXT_DIM)
    } else if inventory.can_craft(recipe) {
        (format!("{have}  -  Craft"), TEXT_CRAFTABLE)
    } else {
        (format!("{have}  -  Missing"), TEXT_BLOCKED)
    }
}

fn owned_status(piece: GearPiece, gear: &PlayerGear) -> (String, Color) {
    let slot = piece.slot.label();
    if gear.is_equipped(&piece) {
        (format!("{slot}  -  Equipped"), TEXT_CRAFTABLE)
    } else {
        (format!("{slot}  -  Equip"), TEXT_PRIMARY)
    }
}

fn row_status(index: usize, inventory: &Inventory, gear: &PlayerGear) -> (String, Color) {
    match row_action(index, &gear.owned) {
        Some(RowAction::Craft(piece)) => recipe_status(piece, inventory, gear),
        Some(RowAction::Equip(piece)) => owned_status(piece, gear),
        None => (String::new(), TEXT_DIM),
    }
}

fn notice_color(notice: &str) -> Color {
    if notice.is_empty() {
        TEXT_DIM
    } else if notice.starts_with("Missing") || notice.contains("already") {
        TEXT_BLOCKED
    } else {
        TEXT_CRAFTABLE
    }
}

fn item_detail(row: &InventoryRow, inventory: &Inventory, gear: &PlayerGear) -> (String, Color) {
    match row {
        InventoryRow::Gear(piece) => {
            let slot = piece.slot.label();
            if gear.is_equipped(piece) {
                (format!("{slot}  -  Equipped"), TEXT_CRAFTABLE)
            } else {
                (format!("{slot}  -  Equip"), TEXT_PRIMARY)
            }
        }
        other => match other.count(inventory) {
            Some(count) => (format!("x{count}"), TEXT_PRIMARY),
            None => (String::new(), TEXT_DIM),
        },
    }
}

// --- Plugin ------------------------------------------------------------------

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        theme::register(app);
        app.init_resource::<InventoryPanel>()
            .init_resource::<CropUnlocks>()
            .init_resource::<BossSelectMenu>()
            .init_resource::<BossProgress>()
            .init_resource::<CropSelectMenu>()
            .add_message::<crate::events::GearCrafted>()
            .add_message::<crate::events::GearEquipped>()
            // The title and victory screens have no world camera of their own.
            .add_systems(OnEnter(GameState::MainMenu), ensure_menu_camera)
            .add_systems(OnExit(GameState::MainMenu), despawn_menu_camera)
            .add_systems(OnEnter(GameState::Victory), ensure_menu_camera)
            .add_systems(OnExit(GameState::Victory), despawn_menu_camera)
            .add_systems(
                OnExit(GameState::Playing),
                (
                    close_menu,
                    despawn_crafting_menu,
                    close_inventory_panel,
                    despawn_inventory_panel,
                    close_boss_select,
                    despawn_boss_select_ui,
                    close_crop_select,
                    despawn_crop_select_ui,
                ),
            )
            .add_systems(
                Update,
                (sync_boss_select, update_boss_concept, refresh_boss_select)
                    .chain()
                    .after(BossSelectSet::Menu)
                    .before(theme::MenuStyleSet),
            )
            .add_systems(
                Update,
                (sync_crop_select, update_plant_preview, refresh_crop_select)
                    .chain()
                    .after(CropSelectSet::Menu)
                    .before(theme::MenuStyleSet),
            )
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
                enforce_single_open_panel
                    .after(InventoryPanelSet::Panel)
                    .after(CraftingMenuSet::Menu)
                    .after(CropSelectSet::Menu),
            )
            .add_systems(
                Update,
                (
                    sync_crafting_menu,
                    update_gear_preview,
                    sync_inventory_panel,
                    refresh_crafting_menu,
                    refresh_inventory_panel,
                )
                    .chain()
                    .after(CraftingMenuSet::Menu)
                    .after(InventoryPanelSet::Panel)
                    .before(theme::MenuStyleSet),
            );
    }
}

// --- Forge (gear) ------------------------------------------------------------

fn spawn_gear_row(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    visuals: theme::MenuButtonVisuals,
    title: String,
    status: (String, Color),
    name_width: f32,
) {
    parent
        .spawn((
            theme::button_core(index, true, false, visuals),
            RecipeRow { index },
        ))
        .with_children(|row| {
            row.spawn((
                MenuText::RowName(index),
                label(title, 17.0, TEXT_PRIMARY),
                Node {
                    width: Val::Px(name_width),
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
            row.spawn((
                MenuText::RowStatus(index),
                label(status.0, 13.0, status.1),
                TextLayout::justify(Justify::Right),
                Node {
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    ..default()
                },
            ));
        });
}

fn spawn_crafting_menu(
    commands: &mut Commands,
    menu: &CraftingMenu,
    inventory: &Inventory,
    gear: &PlayerGear,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
) {
    commands
        .spawn((
            Name::new("Crafting Menu Root"),
            CraftingMenuRoot,
            theme::screen_root_node(),
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Name::new("Crafting Panel"),
                    theme::panel_node(),
                    BackgroundColor(PANEL_BG),
                    BorderColor::all(PANEL_BORDER),
                ))
                .with_children(|panel| {
                    panel.spawn((Name::new("Title"), label("FORGE", 26.0, TEXT_PRIMARY)));
                    panel.spawn((
                        Name::new("Inventory"),
                        MenuText::Line(MenuLine::Inventory),
                        label(format!("Inventory:  {}", inventory.summary()), 14.0, TEXT_DIM),
                    ));
                    panel.spawn((
                        Name::new("Equipped"),
                        MenuText::Line(MenuLine::Equipped),
                        label(gear.equipped_summary(), 14.0, TEXT_PRIMARY),
                    ));
                    panel
                        .spawn((Name::new("Columns"), theme::columns_node()))
                        .with_children(|columns| {
                            // Left: the animated showcase.
                            columns
                                .spawn((
                                    Name::new("Preview"),
                                    preview_pane_node(27.0),
                                    BackgroundColor(Color::srgba(0.04, 0.04, 0.06, 1.0)),
                                    BorderColor::all(ROW_BORDER),
                                ))
                                .with_children(|pane| {
                                    pane.spawn((
                                        Name::new("Gear Preview"),
                                        GearPreview,
                                        AnimatedPreview::new(Vec::new(), None),
                                        ImageNode {
                                            image_mode: NodeImageMode::Auto,
                                            ..default()
                                        },
                                        Node {
                                            width: Val::Px(PREVIEW_SIZE),
                                            height: Val::Px(PREVIEW_SIZE),
                                            ..default()
                                        },
                                    ));
                                });
                            // Middle: every recipe, then owned pieces.
                            columns
                                .spawn((
                                    Name::new("Recipes"),
                                    list_pane_node(41.0),
                                    BackgroundColor(PANEL_BG),
                                    BorderColor::all(ROW_BORDER),
                                ))
                                .with_children(|recipes| {
                                    for index in 0..RECIPE_COUNT {
                                        spawn_gear_row(
                                            recipes,
                                            index,
                                            default_visuals(art, server),
                                            row_title(index, gear),
                                            row_status(index, inventory, gear),
                                            RECIPE_NAME_WIDTH,
                                        );
                                    }
                                    for (offset, _piece) in gear.owned.iter().enumerate() {
                                        let index = RECIPE_COUNT + offset;
                                        spawn_gear_row(
                                            recipes,
                                            index,
                                            default_visuals(art, server),
                                            row_title(index, gear),
                                            row_status(index, inventory, gear),
                                            OWNED_NAME_WIDTH,
                                        );
                                    }
                                });
                            // Right: description and materials.
                            columns
                                .spawn((
                                    Name::new("Detail"),
                                    list_pane_node(32.0),
                                    BackgroundColor(PANEL_BG),
                                    BorderColor::all(ROW_BORDER),
                                ))
                                .with_children(|detail| {
                                    detail.spawn((
                                        Name::new("Description"),
                                        GearDescription,
                                        label("", 15.0, TEXT_PRIMARY),
                                        Node {
                                            margin: UiRect::bottom(Val::Px(8.0)),
                                            ..default()
                                        },
                                    ));
                                    detail.spawn((
                                        Name::new("Materials"),
                                        GearMaterials,
                                        label("", 14.0, TEXT_DIM),
                                    ));
                                });
                        });
                    panel.spawn((
                        Name::new("Notice"),
                        MenuText::Line(MenuLine::Notice),
                        label(menu.notice.clone(), 15.0, notice_color(&menu.notice)),
                    ));
                    panel.spawn((
                        Name::new("Hint"),
                        label(
                            "Arrows select   |   Enter / E / click uses the row   |   Esc or Tab closes",
                            13.0,
                            TEXT_DIM,
                        ),
                    ));
                });
        });
}

/// A pane that centres its single child (the preview art).
fn preview_pane_node(flex_basis: f32) -> Node {
    Node {
        flex_basis: Val::Percent(flex_basis),
        flex_grow: 0.0,
        flex_shrink: 1.0,
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: Val::Px(8.0),
        padding: UiRect::all(Val::Px(12.0)),
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(6.0)),
        ..default()
    }
}

/// A scrolling list pane.
fn list_pane_node(flex_basis: f32) -> Node {
    Node {
        flex_basis: Val::Percent(flex_basis),
        flex_grow: 0.0,
        flex_shrink: 1.0,
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(6.0),
        padding: UiRect::all(Val::Px(10.0)),
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(6.0)),
        overflow: Overflow::scroll_y(),
        ..default()
    }
}

fn despawn_crafting_menu(mut commands: Commands, roots: Query<Entity, With<CraftingMenuRoot>>) {
    for entity in roots.iter() {
        commands.entity(entity).despawn();
    }
}

enum MenuSync {
    Spawn,
    Despawn,
    Rebuild,
    Nothing,
}

#[allow(clippy::too_many_arguments)]
fn sync_crafting_menu(
    mut commands: Commands,
    menu: Res<CraftingMenu>,
    inventory: Res<Inventory>,
    gear: Res<PlayerGear>,
    mut art: ResMut<MenuArt>,
    server: Option<Res<AssetServer>>,
    roots: Query<Entity, With<CraftingMenuRoot>>,
    rows: Query<Entity, With<RecipeRow>>,
) {
    let existing: Vec<Entity> = roots.iter().collect();
    let action = if !menu.open {
        if existing.is_empty() {
            MenuSync::Nothing
        } else {
            MenuSync::Despawn
        }
    } else if existing.is_empty() {
        MenuSync::Spawn
    } else if rows.iter().count() != crate::systems::crafting::row_count(&gear.owned) {
        MenuSync::Rebuild
    } else {
        MenuSync::Nothing
    };

    let server = server.as_deref();
    match action {
        MenuSync::Spawn => {
            spawn_crafting_menu(&mut commands, &menu, &inventory, &gear, &mut art, server)
        }
        MenuSync::Despawn => despawn_crafting_menu(commands, roots),
        MenuSync::Rebuild => {
            despawn_crafting_menu(commands.reborrow(), roots);
            spawn_crafting_menu(&mut commands, &menu, &inventory, &gear, &mut art, server);
        }
        MenuSync::Nothing => {}
    }
}

type GearDescriptionText<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<GearDescription>,
        Without<MenuText>,
        Without<GearMaterials>,
    ),
>;

type GearMaterialsText<'w, 's> = Query<
    'w,
    's,
    &'static mut Text,
    (
        With<GearMaterials>,
        Without<MenuText>,
        Without<GearDescription>,
    ),
>;

fn refresh_crafting_menu(
    menu: Res<CraftingMenu>,
    inventory: Res<Inventory>,
    gear: Res<PlayerGear>,
    mut texts: Query<(&MenuText, &mut Text, &mut TextColor)>,
    mut rows: Query<(&RecipeRow, &mut theme::MenuButtonSelected)>,
    mut description: GearDescriptionText,
    mut materials: GearMaterialsText,
) {
    for (marker, mut text, mut color) in texts.iter_mut() {
        match marker {
            MenuText::Line(MenuLine::Inventory) => {
                *text = Text::new(format!("Inventory:  {}", inventory.summary()));
            }
            MenuText::Line(MenuLine::Equipped) => {
                *text = Text::new(gear.equipped_summary());
            }
            MenuText::Line(MenuLine::Notice) => {
                *text = Text::new(menu.notice.clone());
                color.0 = notice_color(&menu.notice);
            }
            MenuText::RowName(index) => {
                *text = Text::new(row_title(*index, &gear));
            }
            MenuText::RowStatus(index) => {
                let (value, value_color) = row_status(*index, &inventory, &gear);
                *text = Text::new(value);
                color.0 = value_color;
            }
        }
    }
    for (row, mut selected) in rows.iter_mut() {
        selected.0 = row.index == menu.selected;
    }

    let (description_text, material_text) = match selected_piece(&menu, &gear) {
        Some(piece) => {
            let recipe = recipe_for_piece(piece);
            let materials = recipe
                .cost
                .iter()
                .map(|(item, need)| menu_text::cost_line(*item, inventory.count(item), *need))
                .collect::<Vec<String>>()
                .join("\n");
            (
                format!("{}\n{}", piece.name(), menu_text::gear_description(piece)),
                format!("Materials needed:\n{materials}"),
            )
        }
        None => (String::new(), String::new()),
    };
    for mut text in description.iter_mut() {
        *text = Text::new(description_text.clone());
    }
    for mut text in materials.iter_mut() {
        *text = Text::new(material_text.clone());
    }
}

/// Points the forge's left pane at the highlighted piece: armor idles, a
/// weapon alternates its light and heavy swings.
fn update_gear_preview(
    menu: Res<CraftingMenu>,
    gear: Res<PlayerGear>,
    mut previews: Query<&mut AnimatedPreview, With<GearPreview>>,
    cache: Option<ResMut<PlayerSpriteAssets>>,
    layouts: Option<ResMut<Assets<TextureAtlasLayout>>>,
    server: Option<Res<AssetServer>>,
) {
    let (Some(server), Some(mut layouts), Some(mut cache)) = (server, layouts, cache) else {
        return;
    };
    if cache.layout().is_none() {
        let handle = layouts.add(PlayerSpriteAssets::grid_layout());
        cache.set_layout(handle);
    }
    let layout = cache.layout().cloned();

    let Some(piece) = selected_piece(&menu, &gear) else {
        return;
    };
    let look = PlayerLook::from_gear(piece.set);
    let clips: Vec<Handle<Image>> = match piece.slot {
        GearSlot::Armor => vec![cache.image_for(
            SpriteKey::new(look, PlayerAnimState::Idle, Facing8::Down),
            &server,
        )],
        GearSlot::Weapon => vec![
            cache.image_for(
                SpriteKey::new(look, PlayerAnimState::LightAttack, Facing8::Down),
                &server,
            ),
            cache.image_for(
                SpriteKey::new(look, PlayerAnimState::HeavyAttack, Facing8::Down),
                &server,
            ),
        ],
    };
    for mut preview in previews.iter_mut() {
        preview.set_layout(layout.clone());
        preview.set_clips(clips.clone());
    }
}

fn close_menu(mut menu: ResMut<CraftingMenu>) {
    menu.close_menu();
}

// --- Inventory ---------------------------------------------------------------

fn spawn_item_row(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    row: &InventoryRow,
    visuals: theme::MenuButtonVisuals,
) {
    parent
        .spawn((
            theme::button_core(index, true, false, visuals),
            InventorySlot { index },
        ))
        .with_children(|item| {
            item.spawn((
                InventoryText::RowName(index),
                label(row.label(), 16.0, TEXT_PRIMARY),
                Node {
                    width: Val::Px(ITEM_NAME_WIDTH),
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
            item.spawn((
                InventoryText::RowDetail(index),
                label(String::new(), 14.0, TEXT_PRIMARY),
                TextLayout::justify(Justify::Right),
                Node {
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    ..default()
                },
            ));
        });
}

fn spawn_column(
    parent: &mut ChildSpawnerCommands,
    title: &str,
    rows: &[InventoryRow],
    matches: impl Fn(&InventoryRow) -> bool,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
) {
    parent.spawn((
        Name::new(format!("{title} Title")),
        label(title.to_uppercase(), 15.0, TEXT_DIM),
    ));
    let mut column_rows = 0;
    for (index, row) in rows.iter().enumerate() {
        if matches(row) {
            spawn_item_row(parent, index, row, default_visuals(art, server));
            column_rows += 1;
        }
    }
    if column_rows == 0 {
        parent.spawn((Name::new("Empty"), label("none yet", 14.0, TEXT_DIM)));
    }
}

fn spawn_inventory_panel(
    commands: &mut Commands,
    panel: &InventoryPanel,
    inventory: &Inventory,
    gear: &PlayerGear,
    unlocks: &CropUnlocks,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
) {
    let rows = inventory_rows(inventory, gear, unlocks);
    commands
        .spawn((
            Name::new("Inventory Root"),
            InventoryRoot,
            theme::screen_root_node(),
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Name::new("Inventory Panel"),
                    theme::panel_node(),
                    BackgroundColor(PANEL_BG),
                    BorderColor::all(PANEL_BORDER),
                ))
                .with_children(|panel_node| {
                    panel_node.spawn((Name::new("Title"), label("INVENTORY", 26.0, TEXT_PRIMARY)));
                    panel_node.spawn((
                        Name::new("Equipped"),
                        InventoryText::Line(InventoryLine::Equipped),
                        label(gear.equipped_summary(), 14.0, TEXT_DIM),
                    ));
                    panel_node
                        .spawn((Name::new("Columns"), theme::columns_node()))
                        .with_children(|columns| {
                            columns
                                .spawn((Name::new("Crops"), list_pane_node(30.0), BackgroundColor(PANEL_BG), BorderColor::all(ROW_BORDER)))
                                .with_children(|crops| {
                                    spawn_column(crops, "Crops", &rows, |row| {
                                        matches!(row, InventoryRow::Crop(_))
                                    }, art, server);
                                });
                            columns
                                .spawn((Name::new("Materials"), list_pane_node(30.0), BackgroundColor(PANEL_BG), BorderColor::all(ROW_BORDER)))
                                .with_children(|materials| {
                                    spawn_column(materials, "Materials", &rows, |row| {
                                        matches!(row, InventoryRow::Material(_))
                                    }, art, server);
                                });
                            columns
                                .spawn((Name::new("Gear"), list_pane_node(40.0), BackgroundColor(PANEL_BG), BorderColor::all(ROW_BORDER)))
                                .with_children(|gear_column| {
                                    spawn_column(gear_column, "Gear", &rows, |row| {
                                        matches!(row, InventoryRow::Gear(_))
                                    }, art, server);
                                });
                        });
                    panel_node.spawn((
                        Name::new("Notice"),
                        InventoryText::Line(InventoryLine::Notice),
                        label(panel.notice.clone(), 15.0, notice_color(&panel.notice)),
                    ));
                    panel_node.spawn((
                        Name::new("Hint"),
                        label(
                            "Arrows select   |   Enter / E / click equips gear   |   I or Esc closes",
                            13.0,
                            TEXT_DIM,
                        ),
                    ));
                });
        });
}

fn despawn_inventory_panel(mut commands: Commands, roots: Query<Entity, With<InventoryRoot>>) {
    for entity in roots.iter() {
        commands.entity(entity).despawn();
    }
}

#[allow(clippy::too_many_arguments)]
fn sync_inventory_panel(
    mut commands: Commands,
    panel: Res<InventoryPanel>,
    inventory: Res<Inventory>,
    gear: Res<PlayerGear>,
    unlocks: Res<CropUnlocks>,
    mut art: ResMut<MenuArt>,
    server: Option<Res<AssetServer>>,
    roots: Query<Entity, With<InventoryRoot>>,
    slots: Query<Entity, With<InventorySlot>>,
) {
    let existing: Vec<Entity> = roots.iter().collect();
    let action = if !panel.open {
        if existing.is_empty() {
            MenuSync::Nothing
        } else {
            MenuSync::Despawn
        }
    } else if existing.is_empty() {
        MenuSync::Spawn
    } else if slots.iter().count() != inventory_rows(&inventory, &gear, &unlocks).len() {
        MenuSync::Rebuild
    } else {
        MenuSync::Nothing
    };

    let server = server.as_deref();
    match action {
        MenuSync::Spawn => spawn_inventory_panel(
            &mut commands,
            &panel,
            &inventory,
            &gear,
            &unlocks,
            &mut art,
            server,
        ),
        MenuSync::Despawn => despawn_inventory_panel(commands, roots),
        MenuSync::Rebuild => {
            despawn_inventory_panel(commands.reborrow(), roots);
            spawn_inventory_panel(
                &mut commands,
                &panel,
                &inventory,
                &gear,
                &unlocks,
                &mut art,
                server,
            );
        }
        MenuSync::Nothing => {}
    }
}

fn refresh_inventory_panel(
    panel: Res<InventoryPanel>,
    inventory: Res<Inventory>,
    gear: Res<PlayerGear>,
    unlocks: Res<CropUnlocks>,
    mut texts: Query<(&InventoryText, &mut Text, &mut TextColor)>,
    mut rows: Query<(&InventorySlot, &mut theme::MenuButtonSelected)>,
) {
    let panel_rows = inventory_rows(&inventory, &gear, &unlocks);
    for (marker, mut text, mut color) in texts.iter_mut() {
        match marker {
            InventoryText::Line(InventoryLine::Equipped) => {
                *text = Text::new(gear.equipped_summary());
            }
            InventoryText::Line(InventoryLine::Notice) => {
                *text = Text::new(panel.notice.clone());
                color.0 = notice_color(&panel.notice);
            }
            InventoryText::RowName(index) => {
                if let Some(row) = panel_rows.get(*index) {
                    *text = Text::new(row.label());
                }
            }
            InventoryText::RowDetail(index) => {
                if let Some(row) = panel_rows.get(*index) {
                    let (value, value_color) = item_detail(row, &inventory, &gear);
                    *text = Text::new(value);
                    color.0 = value_color;
                }
            }
        }
    }
    for (slot, mut selected) in rows.iter_mut() {
        selected.0 = slot.index == panel.selected;
    }
}

// --- Boss select -------------------------------------------------------------

fn spawn_boss_option_row(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    id: BossId,
    progress: &BossProgress,
    visuals: theme::MenuButtonVisuals,
) {
    parent
        .spawn((
            theme::button_core(index, true, false, visuals),
            BossOption { boss_id: id, index },
        ))
        .with_children(|row| {
            row.spawn((
                BossSelectText::OptionName(index),
                label(id.label(), 20.0, boss_name_color(id, progress)),
                Node {
                    width: Val::Px(180.0),
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
            let (status, color) = boss_status(id, progress);
            row.spawn((
                BossSelectText::OptionStatus(index),
                label(
                    format!("{}   |   {}", menu_text::boss_drops(id), status),
                    13.0,
                    color,
                ),
                TextLayout::justify(Justify::Right),
                Node {
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    ..default()
                },
            ));
        });
}

fn spawn_boss_select(
    commands: &mut Commands,
    progress: &BossProgress,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
) {
    commands
        .spawn((
            Name::new("Boss Select Root"),
            BossSelectRoot,
            theme::screen_root_node(),
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Name::new("Boss Select Panel"),
                    theme::panel_node(),
                    BackgroundColor(PANEL_BG),
                    BorderColor::all(PANEL_BORDER),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Name::new("Title"),
                        BossSelectText::Title,
                        label("SELECT BOSS", 26.0, TEXT_PRIMARY),
                    ));
                    panel
                        .spawn((Name::new("Columns"), theme::columns_node()))
                        .with_children(|columns| {
                            columns
                                .spawn((
                                    Name::new("Concept"),
                                    BossConcept,
                                    preview_pane_node(42.0),
                                    BackgroundColor(Color::srgba(0.04, 0.04, 0.06, 1.0)),
                                    BorderColor::all(ROW_BORDER),
                                ))
                                .with_children(|pane| {
                                    pane.spawn((
                                        Name::new("Concept Row"),
                                        Node {
                                            width: Val::Percent(100.0),
                                            flex_grow: 1.0,
                                            flex_direction: FlexDirection::Row,
                                            column_gap: Val::Px(12.0),
                                            align_items: AlignItems::Center,
                                            justify_content: JustifyContent::Center,
                                            ..default()
                                        },
                                    ))
                                    .with_children(|row| {
                                        for boss in [BossId::BossA, BossId::BossB] {
                                            row.spawn((
                                                Name::new(format!("Concept {boss:?}")),
                                                BossConceptSlot { boss },
                                                AnimatedPreview::still(Handle::default()),
                                                ImageNode {
                                                    image_mode: NodeImageMode::Auto,
                                                    ..default()
                                                },
                                                Node {
                                                    max_width: Val::Px(240.0),
                                                    max_height: Val::Px(240.0),
                                                    flex_grow: 1.0,
                                                    flex_shrink: 1.0,
                                                    ..default()
                                                },
                                            ));
                                        }
                                    });
                                });
                            columns
                                .spawn((
                                    Name::new("Boss List"),
                                    list_pane_node(58.0),
                                    BackgroundColor(PANEL_BG),
                                    BorderColor::all(ROW_BORDER),
                                ))
                                .with_children(|list| {
                                    for (index, id) in BossId::ALL.iter().enumerate() {
                                        let spec = theme::ButtonSpec::new(index, id.label())
                                            .skin(boss_skin(*id));
                                        let visuals = art.resolve(&spec, server);
                                        spawn_boss_option_row(list, index, *id, progress, visuals);
                                    }
                                    // Reserved slot for a future infinite mode.
                                    let spec =
                                        theme::ButtonSpec::new(BossId::ALL.len(), "Infinite Mode")
                                            .label_size(20.0)
                                            .skin("infinite_mode")
                                            .hidden();
                                    let visuals = art.resolve(&spec, server);
                                    theme::spawn_menu_button(list, spec, visuals, (), |_| {});
                                });
                        });
                    panel.spawn((
                        Name::new("Hint"),
                        label(
                            "Arrows select   |   Enter chooses   |   Esc returns to the farm",
                            13.0,
                            TEXT_DIM,
                        ),
                    ));
                });
        });
}

fn spawn_boss_confirm(commands: &mut Commands, menu: &BossSelectMenu) {
    commands
        .spawn((
            Name::new("Boss Confirm Root"),
            BossConfirmRoot,
            theme::screen_root_node(),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Name::new("Boss Confirm Panel"),
                    Node {
                        width: Val::Px(560.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(12.0),
                        padding: UiRect::all(Val::Px(20.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(PANEL_BG),
                    BorderColor::all(PANEL_BORDER),
                ))
                .with_children(|panel| {
                    let prompt = match menu.pending_boss {
                        Some(id) => format!("Enter the {} arena?", id.label()),
                        None => "Enter the arena?".to_string(),
                    };
                    panel.spawn((
                        Name::new("Prompt"),
                        BossSelectText::ConfirmPrompt,
                        label(prompt, 24.0, TEXT_PRIMARY),
                    ));
                    panel.spawn((
                        Name::new("Confirm Hint"),
                        label(
                            "Enter / Space / Y to confirm   |   Esc / N to cancel",
                            14.0,
                            TEXT_DIM,
                        ),
                    ));
                });
        });
}

fn sync_boss_select(
    mut commands: Commands,
    menu: Res<BossSelectMenu>,
    progress: Res<BossProgress>,
    mut art: ResMut<MenuArt>,
    server: Option<Res<AssetServer>>,
    select_roots: Query<Entity, With<BossSelectRoot>>,
    confirm_roots: Query<Entity, With<BossConfirmRoot>>,
) {
    let list_present = !select_roots.is_empty();
    if menu.open && !list_present {
        spawn_boss_select(&mut commands, &progress, &mut art, server.as_deref());
    } else if !menu.open && list_present {
        for entity in select_roots.iter() {
            commands.entity(entity).despawn();
        }
    }

    let confirm_present = !confirm_roots.is_empty();
    if menu.confirmation_open && !confirm_present {
        spawn_boss_confirm(&mut commands, &menu);
    } else if !menu.confirmation_open && confirm_present {
        for entity in confirm_roots.iter() {
            commands.entity(entity).despawn();
        }
    }
}

fn refresh_boss_select(
    menu: Res<BossSelectMenu>,
    progress: Res<BossProgress>,
    mut options: Query<(&BossOption, &mut theme::MenuButtonSelected)>,
    mut texts: Query<(&BossSelectText, &mut Text, &mut TextColor)>,
) {
    let highlighting = menu.open && !menu.confirmation_open;
    for (option, mut selected) in options.iter_mut() {
        selected.0 = highlighting && option.index == menu.selected;
    }
    for (marker, mut text, mut color) in texts.iter_mut() {
        match marker {
            BossSelectText::OptionName(index) => {
                if let Some(id) = BossId::ALL.get(*index) {
                    color.0 = boss_name_color(*id, &progress);
                }
            }
            BossSelectText::OptionStatus(index) => {
                if let Some(id) = BossId::ALL.get(*index) {
                    let (status, value_color) = boss_status(*id, &progress);
                    *text = Text::new(format!("{}   |   {}", menu_text::boss_drops(*id), status));
                    color.0 = value_color;
                }
            }
            _ => {}
        }
    }
}

fn update_boss_concept(
    menu: Res<BossSelectMenu>,
    mut art: ResMut<MenuArt>,
    server: Option<Res<AssetServer>>,
    mut slots: Query<(&BossConceptSlot, &mut AnimatedPreview, &mut Visibility)>,
) {
    if !menu.open {
        return;
    }
    let Some(selected) = BossId::ALL.get(menu.selected).copied() else {
        return;
    };
    let dual = selected == BossId::Dual;
    for (slot, mut preview, mut visibility) in slots.iter_mut() {
        let show = dual || slot.boss == selected;
        *visibility = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if !show {
            continue;
        }
        let handle = art.texture(boss_concept_path(slot.boss), server.as_deref());
        preview.set_layout(None);
        preview.set_clips(vec![handle]);
    }
}

fn close_boss_select(mut menu: ResMut<BossSelectMenu>) {
    menu.close_menu();
}

type BossSelectRoots<'w, 's> =
    Query<'w, 's, Entity, Or<(With<BossSelectRoot>, With<BossConfirmRoot>)>>;

fn despawn_boss_select_ui(mut commands: Commands, roots: BossSelectRoots) {
    for entity in roots.iter() {
        commands.entity(entity).despawn();
    }
}

// --- Planting picker ---------------------------------------------------------

fn spawn_crop_option_row(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    crop: CropType,
    unlocks: &CropUnlocks,
    visuals: theme::MenuButtonVisuals,
) {
    parent
        .spawn((
            theme::button_core(index, true, false, visuals),
            CropOption { crop, index },
        ))
        .with_children(|row| {
            row.spawn((
                CropSelectText::RowName(index),
                label(crop.label(), 20.0, crop_name_color(crop, unlocks)),
                Node {
                    width: Val::Px(180.0),
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
            let (status, color) = crop_status(crop, unlocks);
            row.spawn((
                CropSelectText::RowStatus(index),
                label(status, 14.0, color),
                TextLayout::justify(Justify::Right),
                Node {
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    ..default()
                },
            ));
            row.spawn((
                CropSelectText::RowYield(index),
                label(menu_text::crop_yield(crop), 12.0, TEXT_DIM),
                TextLayout::justify(Justify::Right),
                Node {
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    ..default()
                },
            ));
        });
}

fn spawn_crop_select(
    commands: &mut Commands,
    unlocks: &CropUnlocks,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
) {
    commands
        .spawn((
            Name::new("Crop Select Root"),
            CropSelectRoot,
            theme::screen_root_node(),
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Name::new("Crop Select Panel"),
                    theme::panel_node(),
                    BackgroundColor(PANEL_BG),
                    BorderColor::all(PANEL_BORDER),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Name::new("Title"),
                        CropSelectText::Title,
                        label("PLANT", 26.0, TEXT_PRIMARY),
                    ));
                    panel
                        .spawn((Name::new("Columns"), theme::columns_node()))
                        .with_children(|columns| {
                            columns
                                .spawn((
                                    Name::new("Sapling"),
                                    preview_pane_node(42.0),
                                    BackgroundColor(Color::srgba(0.04, 0.04, 0.06, 1.0)),
                                    BorderColor::all(ROW_BORDER),
                                ))
                                .with_children(|pane| {
                                    pane.spawn((
                                        Name::new("Plant Preview"),
                                        PlantPreview,
                                        AnimatedPreview::new(Vec::new(), None),
                                        ImageNode {
                                            image_mode: NodeImageMode::Auto,
                                            ..default()
                                        },
                                        Node {
                                            width: Val::Px(PREVIEW_SIZE),
                                            height: Val::Px(PREVIEW_SIZE),
                                            ..default()
                                        },
                                    ));
                                });
                            columns
                                .spawn((
                                    Name::new("Crop List"),
                                    list_pane_node(58.0),
                                    BackgroundColor(PANEL_BG),
                                    BorderColor::all(ROW_BORDER),
                                ))
                                .with_children(|list| {
                                    for (index, crop) in CropType::ALL.iter().enumerate() {
                                        let spec = theme::ButtonSpec::new(index, crop.label())
                                            .skin(crop_skin(*crop));
                                        let visuals = art.resolve(&spec, server);
                                        spawn_crop_option_row(list, index, *crop, unlocks, visuals);
                                    }
                                });
                        });
                    panel.spawn((
                        Name::new("Hint"),
                        CropSelectText::Hint,
                        label(
                            "Arrows select   |   Enter / E / click plants   |   Esc cancels",
                            13.0,
                            TEXT_DIM,
                        ),
                    ));
                });
        });
}

fn sync_crop_select(
    mut commands: Commands,
    menu: Res<CropSelectMenu>,
    unlocks: Res<CropUnlocks>,
    mut art: ResMut<MenuArt>,
    server: Option<Res<AssetServer>>,
    roots: Query<Entity, With<CropSelectRoot>>,
) {
    let existing: Vec<Entity> = roots.iter().collect();
    let action = if !menu.open {
        if existing.is_empty() {
            MenuSync::Nothing
        } else {
            MenuSync::Despawn
        }
    } else if existing.is_empty() {
        MenuSync::Spawn
    } else {
        MenuSync::Nothing
    };

    match action {
        MenuSync::Spawn => spawn_crop_select(&mut commands, &unlocks, &mut art, server.as_deref()),
        MenuSync::Despawn => despawn_crop_select_ui(commands, roots),
        MenuSync::Rebuild => {
            despawn_crop_select_ui(commands.reborrow(), roots);
            spawn_crop_select(&mut commands, &unlocks, &mut art, server.as_deref());
        }
        MenuSync::Nothing => {}
    }
}

fn refresh_crop_select(
    menu: Res<CropSelectMenu>,
    unlocks: Res<CropUnlocks>,
    mut options: Query<(&CropOption, &mut theme::MenuButtonSelected)>,
    mut texts: Query<(&CropSelectText, &mut Text, &mut TextColor)>,
) {
    for (option, mut selected) in options.iter_mut() {
        selected.0 = menu.open && option.index == menu.selected;
    }
    for (marker, mut text, mut color) in texts.iter_mut() {
        match marker {
            CropSelectText::RowName(index) => {
                if let Some(crop) = CropType::ALL.get(*index) {
                    color.0 = crop_name_color(*crop, &unlocks);
                }
            }
            CropSelectText::RowStatus(index) => {
                if let Some(crop) = CropType::ALL.get(*index) {
                    let (value, value_color) = crop_status(*crop, &unlocks);
                    *text = Text::new(value);
                    color.0 = value_color;
                }
            }
            CropSelectText::RowYield(index) => {
                if let Some(crop) = CropType::ALL.get(*index) {
                    *text = Text::new(menu_text::crop_yield(*crop));
                }
            }
            _ => {}
        }
    }
}

fn update_plant_preview(
    menu: Res<CropSelectMenu>,
    mut art: ResMut<MenuArt>,
    server: Option<Res<AssetServer>>,
    cache: Option<ResMut<PlayerSpriteAssets>>,
    layouts: Option<ResMut<Assets<TextureAtlasLayout>>>,
    mut previews: Query<&mut AnimatedPreview, With<PlantPreview>>,
) {
    if !menu.open {
        return;
    }
    let Some(crop) = CropType::ALL.get(menu.selected).copied() else {
        return;
    };
    let (Some(server), Some(mut layouts), Some(mut cache)) = (server, layouts, cache) else {
        return;
    };

    // The seedlings are 5x5 sheets, so they share the player/effect grid layout.
    if cache.layout().is_none() {
        let handle = layouts.add(PlayerSpriteAssets::grid_layout());
        cache.set_layout(handle);
    }
    let layout = cache.layout().cloned();
    let sheet = art.texture(plant_path(crop), Some(&server));
    for mut preview in previews.iter_mut() {
        preview.set_layout(layout.clone());
        preview.set_clips(vec![sheet.clone()]);
    }
}

fn close_crop_select(mut menu: ResMut<CropSelectMenu>) {
    menu.close_menu();
}

fn despawn_crop_select_ui(mut commands: Commands, roots: Query<Entity, With<CropSelectRoot>>) {
    for entity in roots.iter() {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::gear::{GearSet, GearSlot, MaterialType};
    use crate::components::pot::CropType;
    use bevy::state::app::StatesPlugin;
    use bevy::transform::TransformPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<CraftingMenu>()
            .init_resource::<Inventory>()
            .init_resource::<PlayerGear>()
            .init_resource::<ButtonInput<KeyCode>>()
            .add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin, UIPlugin))
            .init_state::<GameState>()
            .init_state::<crate::states::DayPhase>();
        app
    }

    fn enter_playing(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
    }

    fn open_menu(app: &mut App) {
        app.world_mut().resource_mut::<CraftingMenu>().open_menu();
        app.update();
    }

    fn close_menu(app: &mut App) {
        app.world_mut().resource_mut::<CraftingMenu>().close_menu();
        app.update();
    }

    fn open_inventory(app: &mut App) {
        app.world_mut()
            .resource_mut::<InventoryPanel>()
            .open_panel();
        app.update();
    }

    fn close_inventory(app: &mut App) {
        app.world_mut()
            .resource_mut::<InventoryPanel>()
            .close_panel();
        app.update();
    }

    fn add_crop(app: &mut App, crop: CropType, amount: u32) {
        app.world_mut()
            .resource_mut::<Inventory>()
            .add_crop(crop, amount);
    }

    fn add_material(app: &mut App, material: MaterialType, amount: u32) {
        app.world_mut()
            .resource_mut::<Inventory>()
            .add_material(material, amount);
    }

    fn own(app: &mut App, set: GearSet, slot: GearSlot) {
        app.world_mut()
            .resource_mut::<PlayerGear>()
            .own(GearPiece::new(set, slot));
    }

    fn root_entities(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<CraftingMenuRoot>>()
            .iter(app.world())
            .collect()
    }

    fn inventory_root_entities(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<InventoryRoot>>()
            .iter(app.world())
            .collect()
    }

    fn item_row_entities(app: &mut App) -> Vec<(usize, Entity)> {
        let mut rows: Vec<(usize, Entity)> = app
            .world_mut()
            .query_filtered::<(Entity, &InventorySlot), With<Button>>()
            .iter(app.world())
            .map(|(entity, slot)| (slot.index, entity))
            .collect();
        rows.sort_by_key(|(index, _)| *index);
        rows
    }

    fn row_entities(app: &mut App) -> Vec<(usize, Entity)> {
        let mut rows: Vec<(usize, Entity)> = app
            .world_mut()
            .query_filtered::<(Entity, &RecipeRow), With<Button>>()
            .iter(app.world())
            .map(|(entity, row)| (row.index, entity))
            .collect();
        rows.sort_by_key(|(index, _)| *index);
        rows
    }

    fn menu_texts(app: &mut App, marker: MenuText) -> Vec<String> {
        app.world_mut()
            .query_filtered::<(&MenuText, &Text), With<MenuText>>()
            .iter(app.world())
            .filter(|(candidate, _)| **candidate == marker)
            .map(|(_, text)| text.0.clone())
            .collect()
    }

    fn line_text(app: &mut App, line: MenuLine) -> Vec<String> {
        menu_texts(app, MenuText::Line(line))
    }

    fn row_text(app: &mut App, marker_index: usize) -> Vec<String> {
        menu_texts(app, MenuText::RowName(marker_index))
    }

    fn status_text(app: &mut App, marker_index: usize) -> Vec<String> {
        menu_texts(app, MenuText::RowStatus(marker_index))
    }

    fn inventory_texts(app: &mut App, marker: InventoryText) -> Vec<String> {
        app.world_mut()
            .query_filtered::<(&InventoryText, &Text), With<InventoryText>>()
            .iter(app.world())
            .filter(|(candidate, _)| **candidate == marker)
            .map(|(_, text)| text.0.clone())
            .collect()
    }

    fn item_name(app: &mut App, index: usize) -> Vec<String> {
        inventory_texts(app, InventoryText::RowName(index))
    }

    fn item_detail_text(app: &mut App, index: usize) -> Vec<String> {
        inventory_texts(app, InventoryText::RowDetail(index))
    }

    fn notice_text(app: &mut App) -> Vec<String> {
        inventory_texts(app, InventoryText::Line(InventoryLine::Notice))
    }

    fn equipped_text(app: &mut App) -> Vec<String> {
        inventory_texts(app, InventoryText::Line(InventoryLine::Equipped))
    }

    fn gear_with_starter_set() -> PlayerGear {
        let mut gear = PlayerGear::default();
        for piece in [
            GearPiece::new(GearSet::Starter, GearSlot::Weapon),
            GearPiece::new(GearSet::Starter, GearSlot::Armor),
        ] {
            gear.own(piece);
        }
        gear
    }

    #[test]
    fn ui_plugin_exists() {
        let _plugin = UIPlugin;
    }

    #[test]
    fn nothing_is_spawned_while_the_menu_is_closed() {
        let mut app = setup_app();
        enter_playing(&mut app);
        assert!(root_entities(&mut app).is_empty());
        assert!(row_entities(&mut app).is_empty());
    }

    #[test]
    fn opening_the_menu_spawns_a_single_root() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);

        assert_eq!(root_entities(&mut app).len(), 1);
    }

    #[test]
    fn menu_does_not_spawn_twice_while_open() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);
        app.update();
        app.update();
        assert_eq!(root_entities(&mut app).len(), 1);
    }

    #[test]
    fn root_fills_the_screen() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);
        let root = root_entities(&mut app)[0];

        let node = app.world().get::<Node>(root).expect("root node");
        assert_eq!(node.position_type, PositionType::Absolute);
        assert_eq!(node.width, Val::Percent(100.0));
        assert_eq!(node.height, Val::Percent(100.0));
    }

    #[test]
    fn closing_the_menu_despawns_every_row() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);
        assert_eq!(row_entities(&mut app).len(), RECIPE_COUNT);

        close_menu(&mut app);

        assert!(root_entities(&mut app).is_empty());
        assert!(row_entities(&mut app).is_empty());
    }

    #[test]
    fn the_menu_can_be_reopened_after_closing() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);
        close_menu(&mut app);
        open_menu(&mut app);

        assert_eq!(root_entities(&mut app).len(), 1);
        assert_eq!(row_entities(&mut app).len(), RECIPE_COUNT);
    }

    #[test]
    fn one_clickable_row_per_recipe_in_order() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);

        let rows = row_entities(&mut app);
        assert_eq!(rows.len(), RECIPE_COUNT);
        for (position, (index, entity)) in rows.iter().enumerate() {
            assert_eq!(*index, position);
            assert!(app.world().get::<Interaction>(*entity).is_some());
            assert!(app.world().get::<Button>(*entity).is_some());
            assert_eq!(
                app.world().get::<FocusPolicy>(*entity),
                Some(&FocusPolicy::Block)
            );
        }
    }

    #[test]
    fn owned_pieces_append_rows_after_the_recipes() {
        let mut app = setup_app();
        enter_playing(&mut app);
        *app.world_mut().resource_mut::<PlayerGear>() = gear_with_starter_set();
        open_menu(&mut app);

        let rows = row_entities(&mut app);
        assert_eq!(rows.len(), RECIPE_COUNT + 2);
        assert_eq!(rows[RECIPE_COUNT].0, RECIPE_COUNT);
        assert_eq!(rows[RECIPE_COUNT + 1].0, RECIPE_COUNT + 1);
    }

    #[test]
    fn gaining_pieces_while_open_grows_the_menu() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);
        assert_eq!(row_entities(&mut app).len(), RECIPE_COUNT);

        let piece = GearPiece::new(GearSet::Starter, GearSlot::Weapon);
        app.world_mut().resource_mut::<PlayerGear>().own(piece);
        app.update();

        let rows = row_entities(&mut app);
        assert_eq!(rows.len(), RECIPE_COUNT + 1);
        assert_eq!(rows.last().unwrap().0, RECIPE_COUNT);
        assert_eq!(
            row_text(&mut app, RECIPE_COUNT),
            vec!["Starter Spearblade".to_string()]
        );
    }

    #[test]
    fn the_menu_stays_stable_when_nothing_changes() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);
        let root = root_entities(&mut app)[0];

        app.update();
        app.update();

        assert_eq!(root_entities(&mut app), vec![root]);
    }

    #[test]
    fn every_recipe_gets_a_titled_row() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);

        for (index, set) in GearSet::ALL.iter().enumerate() {
            for (slot_index, piece) in set.pieces().iter().enumerate() {
                let row = index * 2 + slot_index;
                assert_eq!(row_text(&mut app, row), vec![piece.name().to_string()]);
            }
        }
    }

    #[test]
    fn recipe_rows_show_have_over_need_counts() {
        let mut app = setup_app();
        enter_playing(&mut app);
        app.world_mut()
            .resource_mut::<Inventory>()
            .add_crop(CropType::Starter, 12);
        open_menu(&mut app);

        let statuses = status_text(&mut app, 0);
        assert_eq!(statuses.len(), 1);
        assert!(statuses[0].contains("Starter Crop 12/2"));
        assert!(statuses[0].contains("Craft"));
    }

    #[test]
    fn locked_recipes_are_marked_missing() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);

        let statuses = status_text(&mut app, 2);
        assert!(statuses[0].contains("Crop A 0/2"));
        assert!(statuses[0].contains("Boss A Material 1 0/1"));
        assert!(statuses[0].contains("Missing"));
    }

    #[test]
    fn owned_pieces_are_marked_owned_instead_of_craftable() {
        let mut app = setup_app();
        enter_playing(&mut app);
        app.world_mut()
            .resource_mut::<Inventory>()
            .add_crop(CropType::Starter, 40);
        *app.world_mut().resource_mut::<PlayerGear>() = gear_with_starter_set();
        open_menu(&mut app);

        let statuses = status_text(&mut app, 0);
        assert!(statuses[0].contains("Owned"));
        assert!(!statuses[0].contains("Craft"));
    }

    #[test]
    fn inventory_line_reflects_the_inventory() {
        let mut app = setup_app();
        enter_playing(&mut app);
        app.world_mut()
            .resource_mut::<Inventory>()
            .add_crop(CropType::CropA, 3);
        open_menu(&mut app);

        let lines = line_text(&mut app, MenuLine::Inventory);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("Inventory:"));
        assert!(lines[0].contains("Crop A 3"));
    }

    #[test]
    fn inventory_line_updates_when_the_inventory_changes() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);

        app.world_mut()
            .resource_mut::<Inventory>()
            .add_crop(CropType::Starter, 9);
        app.update();

        let lines = line_text(&mut app, MenuLine::Inventory);
        assert!(lines[0].contains("Starter Crop 9"));
    }

    #[test]
    fn equipped_line_starts_with_placeholders() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);

        let lines = line_text(&mut app, MenuLine::Equipped);
        assert!(lines[0].contains("Weapon: -"));
        assert!(lines[0].contains("Armor: -"));
    }

    #[test]
    fn equipped_line_updates_after_equipping() {
        let mut app = setup_app();
        enter_playing(&mut app);
        let piece = GearPiece::new(GearSet::Starter, GearSlot::Weapon);
        {
            let mut gear = app.world_mut().resource_mut::<PlayerGear>();
            gear.own(piece);
            gear.equip(&piece);
        }
        open_menu(&mut app);

        let lines = line_text(&mut app, MenuLine::Equipped);
        assert!(lines[0].contains("Weapon: Starter Spearblade"));
        assert!(lines[0].contains("Armor: -"));
    }

    #[test]
    fn owned_rows_show_slot_and_equip_state() {
        let mut app = setup_app();
        enter_playing(&mut app);
        let mut gear = gear_with_starter_set();
        gear.equip(&GearPiece::new(GearSet::Starter, GearSlot::Weapon));
        *app.world_mut().resource_mut::<PlayerGear>() = gear;
        open_menu(&mut app);

        assert!(status_text(&mut app, RECIPE_COUNT)[0].contains("Weapon  -  Equipped"));
        assert!(status_text(&mut app, RECIPE_COUNT + 1)[0].contains("Armor  -  Equip"));
        assert_eq!(
            row_text(&mut app, RECIPE_COUNT),
            vec!["Starter Spearblade".to_string()]
        );
    }

    #[test]
    fn notice_line_shows_the_last_action() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);

        app.world_mut()
            .resource_mut::<CraftingMenu>()
            .set_notice("Missing: Crop B x5");
        app.update();

        assert_eq!(
            line_text(&mut app, MenuLine::Notice),
            vec!["Missing: Crop B x5".to_string()]
        );
    }

    #[test]
    fn selected_row_gets_the_highlighted_border() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);
        app.world_mut().resource_mut::<CraftingMenu>().selected = 2;
        app.update();

        let rows = row_entities(&mut app);
        assert_eq!(
            app.world().get::<BorderColor>(rows[2].1).unwrap().top,
            ROW_BORDER_SELECTED
        );
        assert_eq!(
            app.world().get::<BorderColor>(rows[0].1).unwrap().top,
            ROW_BORDER
        );
    }

    #[test]
    fn hovering_a_row_lightens_its_background() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);
        let (_, row) = row_entities(&mut app)[1];

        *app.world_mut().get_mut::<Interaction>(row).unwrap() = Interaction::Hovered;
        app.update();

        assert_eq!(
            app.world().get::<BackgroundColor>(row).unwrap().0,
            ROW_BG_HOVER
        );
    }

    #[test]
    fn leaving_the_row_restores_its_background() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);
        let (_, row) = row_entities(&mut app)[1];

        *app.world_mut().get_mut::<Interaction>(row).unwrap() = Interaction::Hovered;
        app.update();
        *app.world_mut().get_mut::<Interaction>(row).unwrap() = Interaction::None;
        app.update();

        assert_eq!(app.world().get::<BackgroundColor>(row).unwrap().0, ROW_BG);
    }

    #[test]
    fn leaving_playing_despawns_the_menu() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);
        assert_eq!(root_entities(&mut app).len(), 1);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();

        assert!(root_entities(&mut app).is_empty());
    }

    #[test]
    fn notice_colours_distinguish_blocked_from_success() {
        assert_eq!(notice_color(""), TEXT_DIM);
        assert_eq!(notice_color("Missing: Crop B x5"), TEXT_BLOCKED);
        assert_eq!(
            notice_color("Starter Spearblade already owned"),
            TEXT_BLOCKED
        );
        assert_eq!(notice_color("Crafted Starter Spearblade"), TEXT_CRAFTABLE);
    }

    #[test]
    fn recipe_status_line_covers_every_state() {
        let mut inventory = Inventory::default();
        inventory.add_crop(CropType::Starter, 12);
        assert!(
            recipe_status(
                GearPiece::new(GearSet::Starter, GearSlot::Weapon),
                &inventory,
                &PlayerGear::default()
            )
            .0
            .contains("Craft")
        );

        let gear = gear_with_starter_set();
        assert!(
            recipe_status(
                GearPiece::new(GearSet::Starter, GearSlot::Weapon),
                &inventory,
                &gear
            )
            .0
            .contains("Owned")
        );

        let (blocked, color) = recipe_status(
            GearPiece::new(GearSet::Starter, GearSlot::Weapon),
            &Inventory::default(),
            &PlayerGear::default(),
        );
        assert!(blocked.contains("Starter Crop 0/2"));
        assert!(blocked.contains("Missing"));
        assert_eq!(color, TEXT_BLOCKED);
    }

    #[test]
    fn master_rows_list_every_cost_line() {
        let mut inventory = Inventory::default();
        inventory.add_crop(CropType::Starter, 1);
        inventory.add_material(MaterialType::BossA1, 2);
        let (status, _) = recipe_status(
            GearPiece::new(GearSet::Master, GearSlot::Weapon),
            &inventory,
            &PlayerGear::default(),
        );

        assert!(status.contains("Starter Crop 1/1"));
        assert!(status.contains("Crop A 0/1"));
        assert!(status.contains("Crop B 0/1"));
        assert!(status.contains("Boss A Material 1 2/2"));
        assert!(status.contains("Boss B Material 1 0/2"));
    }

    #[test]
    fn owned_status_line_marks_the_equipped_piece() {
        let piece = GearPiece::new(GearSet::Starter, GearSlot::Armor);
        let mut gear = gear_with_starter_set();
        assert_eq!(owned_status(piece, &gear).0, "Armor  -  Equip");
        gear.equip(&piece);
        assert_eq!(owned_status(piece, &gear).0, "Armor  -  Equipped");
    }

    #[test]
    fn row_title_falls_back_to_empty_for_unknown_rows() {
        assert_eq!(row_title(0, &PlayerGear::default()), "Starter Spearblade");
        assert_eq!(row_title(RECIPE_COUNT, &PlayerGear::default()), "");
    }

    #[test]
    fn no_inventory_ui_is_spawned_while_the_panel_is_closed() {
        let mut app = setup_app();
        enter_playing(&mut app);
        assert!(inventory_root_entities(&mut app).is_empty());
        assert!(item_row_entities(&mut app).is_empty());
    }

    #[test]
    fn opening_the_panel_spawns_a_single_full_screen_root() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_inventory(&mut app);

        let roots = inventory_root_entities(&mut app);
        assert_eq!(roots.len(), 1);
        let node = app.world().get::<Node>(roots[0]).expect("root node");
        assert_eq!(node.position_type, PositionType::Absolute);
        assert_eq!(node.width, Val::Percent(100.0));
        assert_eq!(node.height, Val::Percent(100.0));
    }

    #[test]
    fn the_inventory_panel_does_not_spawn_twice() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_inventory(&mut app);
        app.update();
        app.update();
        assert_eq!(inventory_root_entities(&mut app).len(), 1);
    }

    #[test]
    fn closing_the_panel_despawns_every_row() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_inventory(&mut app);
        assert!(!item_row_entities(&mut app).is_empty());

        close_inventory(&mut app);

        assert!(inventory_root_entities(&mut app).is_empty());
        assert!(item_row_entities(&mut app).is_empty());
    }

    #[test]
    fn the_panel_can_be_reopened_after_closing() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_inventory(&mut app);
        close_inventory(&mut app);
        open_inventory(&mut app);

        assert_eq!(inventory_root_entities(&mut app).len(), 1);
    }

    #[test]
    fn only_the_starter_crop_row_shows_at_the_start() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_inventory(&mut app);

        assert_eq!(
            item_row_entities(&mut app),
            vec![(0, item_row_entities(&mut app)[0].1)]
        );
        assert_eq!(item_name(&mut app, 0), vec!["Starter Crop".to_string()]);
        assert_eq!(item_detail_text(&mut app, 0), vec!["x0".to_string()]);
    }

    #[test]
    fn unlocked_crops_gain_a_row() {
        let mut app = setup_app();
        enter_playing(&mut app);
        app.world_mut()
            .resource_mut::<CropUnlocks>()
            .unlock_crop_a();
        open_inventory(&mut app);

        assert_eq!(item_row_entities(&mut app).len(), 2);
        assert_eq!(item_name(&mut app, 1), vec!["Crop A".to_string()]);
    }

    #[test]
    fn stocked_materials_gain_a_row_while_others_stay_hidden() {
        let mut app = setup_app();
        enter_playing(&mut app);
        add_material(&mut app, MaterialType::BossA1, 3);
        open_inventory(&mut app);

        assert_eq!(item_row_entities(&mut app).len(), 2);
        assert_eq!(
            item_name(&mut app, 1),
            vec!["Boss A Material 1".to_string()]
        );
        assert_eq!(item_detail_text(&mut app, 1), vec!["x3".to_string()]);
    }

    #[test]
    fn growing_the_inventory_rebuilds_the_rows() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_inventory(&mut app);
        assert_eq!(item_row_entities(&mut app).len(), 1);

        add_crop(&mut app, CropType::Starter, 4);
        app.update();
        add_material(&mut app, MaterialType::BossB1, 1);
        app.update();

        assert_eq!(inventory_root_entities(&mut app).len(), 1);
        assert_eq!(item_row_entities(&mut app).len(), 2);
        assert_eq!(
            item_name(&mut app, 1),
            vec!["Boss B Material 1".to_string()]
        );
    }

    #[test]
    fn owned_gear_adds_clickable_rows_after_the_items() {
        let mut app = setup_app();
        enter_playing(&mut app);
        own(&mut app, GearSet::Master, GearSlot::Weapon);
        open_inventory(&mut app);

        let rows = item_row_entities(&mut app);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].0, 1);
        let entity = rows[1].1;
        assert!(app.world().get::<Button>(entity).is_some());
        assert!(app.world().get::<Interaction>(entity).is_some());
        assert_eq!(
            app.world().get::<FocusPolicy>(entity),
            Some(&FocusPolicy::Block)
        );
        assert_eq!(
            item_name(&mut app, 1),
            vec!["Dreaming Spearblade (Master Set)".to_string()]
        );
        assert_eq!(
            item_detail_text(&mut app, 1),
            vec!["Weapon  -  Equip".to_string()]
        );
    }

    #[test]
    fn gear_rows_mark_the_equipped_piece() {
        let mut app = setup_app();
        enter_playing(&mut app);
        let piece = GearPiece::new(GearSet::Master, GearSlot::Weapon);
        own(&mut app, GearSet::Master, GearSlot::Weapon);
        app.world_mut().resource_mut::<PlayerGear>().equip(&piece);
        open_inventory(&mut app);

        assert_eq!(
            item_detail_text(&mut app, 1),
            vec!["Weapon  -  Equipped".to_string()]
        );
    }

    #[test]
    fn row_counts_refresh_without_respawning() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_inventory(&mut app);
        let root = inventory_root_entities(&mut app)[0];

        add_crop(&mut app, CropType::Starter, 11);
        app.update();

        assert_eq!(inventory_root_entities(&mut app), vec![root]);
        assert_eq!(item_detail_text(&mut app, 0), vec!["x11".to_string()]);
    }

    #[test]
    fn the_selected_row_gets_the_highlighted_border() {
        let mut app = setup_app();
        enter_playing(&mut app);
        add_material(&mut app, MaterialType::BossA1, 1);
        own(&mut app, GearSet::Starter, GearSlot::Armor);
        open_inventory(&mut app);
        app.world_mut().resource_mut::<InventoryPanel>().selected = 2;
        app.update();

        let rows = item_row_entities(&mut app);
        assert_eq!(
            app.world().get::<BorderColor>(rows[2].1).unwrap().top,
            ROW_BORDER_SELECTED
        );
        assert_eq!(
            app.world().get::<BorderColor>(rows[0].1).unwrap().top,
            ROW_BORDER
        );
    }

    #[test]
    fn hovering_an_item_row_lightens_its_background() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_inventory(&mut app);
        let (_, entity) = item_row_entities(&mut app)[0];

        *app.world_mut().get_mut::<Interaction>(entity).unwrap() = Interaction::Hovered;
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(entity).unwrap().0,
            ROW_BG_HOVER
        );

        *app.world_mut().get_mut::<Interaction>(entity).unwrap() = Interaction::None;
        app.update();
        assert_eq!(
            app.world().get::<BackgroundColor>(entity).unwrap().0,
            ROW_BG
        );
    }

    #[test]
    fn the_inventory_notice_line_shows_the_last_action() {
        let mut app = setup_app();
        enter_playing(&mut app);
        own(&mut app, GearSet::Starter, GearSlot::Weapon);
        open_inventory(&mut app);

        app.world_mut()
            .resource_mut::<InventoryPanel>()
            .set_notice("Equipped Starter Spearblade (Starter Set)");
        app.update();

        assert_eq!(
            notice_text(&mut app),
            vec!["Equipped Starter Spearblade (Starter Set)".to_string()]
        );
    }

    #[test]
    fn the_equipped_line_updates_when_gear_changes() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_inventory(&mut app);
        assert!(equipped_text(&mut app)[0].contains("Weapon: -"));

        let piece = GearPiece::new(GearSet::Starter, GearSlot::Weapon);
        own(&mut app, GearSet::Starter, GearSlot::Weapon);
        app.world_mut().resource_mut::<PlayerGear>().equip(&piece);
        app.update();

        assert!(equipped_text(&mut app)[0].contains("Weapon: Starter Spearblade"));
    }

    #[test]
    fn opening_the_inventory_removes_the_crafting_menu() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);
        assert_eq!(root_entities(&mut app).len(), 1);

        open_inventory(&mut app);

        assert!(root_entities(&mut app).is_empty());
        assert_eq!(inventory_root_entities(&mut app).len(), 1);
    }

    #[test]
    fn leaving_playing_despawns_the_inventory_panel() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_inventory(&mut app);
        assert_eq!(inventory_root_entities(&mut app).len(), 1);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();

        assert!(inventory_root_entities(&mut app).is_empty());
        assert!(item_row_entities(&mut app).is_empty());
    }

    #[test]
    fn running_frames_with_the_panel_open_keeps_it_stable() {
        let mut app = setup_app();
        enter_playing(&mut app);
        add_material(&mut app, MaterialType::BossA1, 2);
        open_inventory(&mut app);
        let root = inventory_root_entities(&mut app)[0];

        for _ in 0..20 {
            app.update();
        }

        assert_eq!(inventory_root_entities(&mut app), vec![root]);
        assert_eq!(item_row_entities(&mut app).len(), 2);
    }

    #[test]
    fn item_detail_line_covers_every_row_kind() {
        let mut inventory = Inventory::default();
        inventory.add_material(MaterialType::BossB1, 5);
        let gear = gear_with_starter_set();

        let crop = InventoryRow::Crop(CropType::Starter);
        assert_eq!(item_detail(&crop, &inventory, &gear).0, "x0");
        assert_eq!(item_detail(&crop, &inventory, &gear).1, TEXT_PRIMARY);

        let material = InventoryRow::Material(MaterialType::BossB1);
        assert_eq!(item_detail(&material, &inventory, &gear).0, "x5");

        let armor = InventoryRow::Gear(GearPiece::new(GearSet::Starter, GearSlot::Armor));
        assert_eq!(
            item_detail(&armor, &inventory, &gear),
            ("Armor  -  Equip".to_string(), TEXT_PRIMARY)
        );

        let mut equipped_gear = gear.clone();
        equipped_gear.equip(&GearPiece::new(GearSet::Starter, GearSlot::Weapon));
        let weapon = InventoryRow::Gear(GearPiece::new(GearSet::Starter, GearSlot::Weapon));
        assert_eq!(
            item_detail(&weapon, &inventory, &equipped_gear),
            ("Weapon  -  Equipped".to_string(), TEXT_CRAFTABLE)
        );
    }

    fn crop_root_entities(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<CropSelectRoot>>()
            .iter(app.world())
            .collect()
    }

    fn crop_option_entities(app: &mut App) -> Vec<(usize, Entity)> {
        let mut rows: Vec<(usize, Entity)> = app
            .world_mut()
            .query_filtered::<(Entity, &CropOption), With<Button>>()
            .iter(app.world())
            .map(|(entity, option)| (option.index, entity))
            .collect();
        rows.sort_by_key(|(index, _)| *index);
        rows
    }

    fn crop_select_texts(app: &mut App, marker: CropSelectText) -> Vec<String> {
        app.world_mut()
            .query_filtered::<(&CropSelectText, &Text), With<CropSelectText>>()
            .iter(app.world())
            .filter(|(candidate, _)| **candidate == marker)
            .map(|(_, text)| text.0.clone())
            .collect()
    }

    fn open_crop_select(app: &mut App, pot: usize) {
        app.world_mut()
            .resource_mut::<CropSelectMenu>()
            .open_menu(pot);
        app.update();
    }

    #[test]
    fn the_crop_picker_spawns_one_root_while_open() {
        let mut app = setup_app();
        enter_playing(&mut app);
        assert!(crop_root_entities(&mut app).is_empty());

        open_crop_select(&mut app, 0);

        assert_eq!(crop_root_entities(&mut app).len(), 1);
    }

    #[test]
    fn the_crop_picker_does_not_spawn_twice() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_crop_select(&mut app, 0);
        app.update();
        app.update();

        assert_eq!(crop_root_entities(&mut app).len(), 1);
    }

    #[test]
    fn the_crop_picker_lists_every_crop_in_order() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_crop_select(&mut app, 0);

        let rows = crop_option_entities(&mut app);
        assert_eq!(rows.len(), CropType::ALL.len());
        for (position, (index, entity)) in rows.iter().enumerate() {
            assert_eq!(*index, position);
            assert!(app.world().get::<Interaction>(*entity).is_some());
            assert!(app.world().get::<Button>(*entity).is_some());
        }
        assert_eq!(
            crop_select_texts(&mut app, CropSelectText::RowName(0)),
            vec!["Starter Crop".to_string()]
        );
    }

    #[test]
    fn locked_crops_are_marked_locked() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_crop_select(&mut app, 0);

        assert_eq!(
            crop_select_texts(&mut app, CropSelectText::RowStatus(0)),
            vec!["1 days".to_string()]
        );
        assert_eq!(
            crop_select_texts(&mut app, CropSelectText::RowStatus(1)),
            vec!["LOCKED  -  Beat Boss A".to_string()]
        );
        assert_eq!(
            crop_select_texts(&mut app, CropSelectText::RowStatus(2)),
            vec!["LOCKED  -  Beat Boss B".to_string()]
        );
    }

    #[test]
    fn unlocking_a_crop_updates_its_status_without_respawning() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_crop_select(&mut app, 0);
        let root = crop_root_entities(&mut app)[0];

        app.world_mut()
            .resource_mut::<CropUnlocks>()
            .unlock_crop_a();
        app.update();

        assert_eq!(crop_root_entities(&mut app), vec![root]);
        assert_eq!(
            crop_select_texts(&mut app, CropSelectText::RowStatus(1)),
            vec!["2 days".to_string()]
        );
    }

    #[test]
    fn the_selected_crop_gets_the_highlighted_border() {
        let mut app = setup_app();
        enter_playing(&mut app);
        app.world_mut()
            .resource_mut::<CropUnlocks>()
            .unlock_crop_a();
        open_crop_select(&mut app, 0);
        app.world_mut().resource_mut::<CropSelectMenu>().selected = 1;
        app.update();

        let rows = crop_option_entities(&mut app);
        assert_eq!(
            app.world().get::<BorderColor>(rows[1].1).unwrap().top,
            ROW_BORDER_SELECTED
        );
        assert_eq!(
            app.world().get::<BorderColor>(rows[0].1).unwrap().top,
            ROW_BORDER
        );
    }

    #[test]
    fn closing_the_crop_picker_despawns_it() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_crop_select(&mut app, 0);
        assert_eq!(crop_root_entities(&mut app).len(), 1);

        app.world_mut()
            .resource_mut::<CropSelectMenu>()
            .close_menu();
        app.update();

        assert!(crop_root_entities(&mut app).is_empty());
        assert!(crop_option_entities(&mut app).is_empty());
    }

    #[test]
    fn leaving_playing_despawns_the_crop_picker() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_crop_select(&mut app, 0);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();

        assert!(crop_root_entities(&mut app).is_empty());
    }

    #[test]
    fn crop_status_distinguishes_locked_and_unlocked() {
        let mut unlocks = CropUnlocks::new();
        assert_eq!(crop_status(CropType::Starter, &unlocks).0, "1 days");
        assert_eq!(
            crop_status(CropType::CropA, &unlocks),
            ("LOCKED  -  Beat Boss A".to_string(), TEXT_BLOCKED)
        );

        unlocks.unlock_crop_a();
        assert_eq!(crop_status(CropType::CropA, &unlocks).0, "2 days");
        assert_eq!(crop_name_color(CropType::CropA, &unlocks), TEXT_PRIMARY);
        assert_eq!(crop_name_color(CropType::CropB, &unlocks), TEXT_DIM);
    }

    fn boss_option_entities(app: &mut App) -> Vec<(usize, Entity)> {
        let mut rows: Vec<(usize, Entity)> = app
            .world_mut()
            .query_filtered::<(Entity, &BossOption), With<Button>>()
            .iter(app.world())
            .map(|(entity, option)| (option.index, entity))
            .collect();
        rows.sort_by_key(|(index, _)| *index);
        rows
    }

    #[test]
    fn the_boss_select_pane_shows_concept_art_and_a_drop_list() {
        let mut app = setup_app();
        enter_playing(&mut app);
        app.world_mut().resource_mut::<BossSelectMenu>().open_menu();
        app.update();

        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<BossConcept>>()
                .iter(app.world())
                .count(),
            1,
            "one concept-art pane"
        );
        let rows = boss_option_entities(&mut app);
        assert_eq!(rows.len(), BossId::ALL.len());
        let statuses = app
            .world_mut()
            .query_filtered::<(&BossSelectText, &Text), With<BossSelectText>>()
            .iter(app.world())
            .filter(|(marker, _)| **marker == BossSelectText::OptionStatus(0))
            .map(|(_, text)| text.0.clone())
            .collect::<Vec<String>>();
        assert!(statuses[0].contains("Drops:"), "{}", statuses[0]);
    }

    #[test]
    fn the_dual_boss_shows_both_concepts() {
        let mut app = setup_app();
        enter_playing(&mut app);
        app.world_mut().resource_mut::<BossSelectMenu>().open_menu();
        app.update();
        app.world_mut().resource_mut::<BossSelectMenu>().selected = 2;
        app.update();

        let visible: Vec<Visibility> = app
            .world_mut()
            .query_filtered::<&Visibility, With<BossConceptSlot>>()
            .iter(app.world())
            .copied()
            .collect();
        assert_eq!(visible.len(), 2, "one slot per half");
        assert!(visible.iter().all(|v| *v == Visibility::Inherited));

        // A single boss leaves the other half hidden.
        app.world_mut().resource_mut::<BossSelectMenu>().selected = 0;
        app.update();
        let visible: Vec<Visibility> = app
            .world_mut()
            .query_filtered::<&Visibility, With<BossConceptSlot>>()
            .iter(app.world())
            .copied()
            .collect();
        assert_eq!(
            visible
                .iter()
                .filter(|v| **v == Visibility::Inherited)
                .count(),
            1
        );
    }

    #[test]
    fn the_planting_pane_shows_a_sapling_and_yield_lines() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_crop_select(&mut app, 0);

        assert_eq!(
            app.world_mut()
                .query_filtered::<Entity, With<PlantPreview>>()
                .iter(app.world())
                .count(),
            1,
            "one sapling pane"
        );
        assert_eq!(
            crop_select_texts(&mut app, CropSelectText::RowYield(0)),
            vec!["Yields: Starter Crop x1".to_string()]
        );
    }

    #[test]
    fn the_full_screen_menus_spawn_and_drop_their_own_camera() {
        let mut app = setup_app();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::MainMenu);
        app.update();
        app.update();

        let cameras = |app: &mut App| {
            app.world_mut()
                .query_filtered::<Entity, With<MenuCamera>>()
                .iter(app.world())
                .count()
        };
        assert_eq!(cameras(&mut app), 1, "the title screen needs a camera");

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        app.update();
        assert_eq!(cameras(&mut app), 0, "the menu camera yields to play");
    }
}
