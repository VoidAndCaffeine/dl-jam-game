use crate::components::gear::{GearPiece, GearSet, RECIPE_COUNT, recipe_for_set};
use crate::resources::crafting_menu::CraftingMenu;
use crate::resources::inventory::Inventory;
use crate::resources::run_data::PlayerGear;
use crate::states::GameState;
use crate::systems::crafting::{CraftingMenuSet, RecipeRow, RowAction, row_action};
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

const PANEL_WIDTH: f32 = 940.0;
const ROW_HEIGHT: f32 = 32.0;
const RECIPE_NAME_WIDTH: f32 = 210.0;
const OWNED_NAME_WIDTH: f32 = 180.0;

const PANEL_BG: Color = Color::srgba(0.07, 0.07, 0.10, 0.96);
const PANEL_BORDER: Color = Color::srgba(0.55, 0.55, 0.68, 1.0);
const TEXT_PRIMARY: Color = Color::srgba(0.94, 0.94, 0.97, 1.0);
const TEXT_DIM: Color = Color::srgba(0.62, 0.63, 0.70, 1.0);
const TEXT_CRAFTABLE: Color = Color::srgba(0.44, 0.95, 0.52, 1.0);
const TEXT_BLOCKED: Color = Color::srgba(1.0, 0.76, 0.36, 1.0);
const ROW_BG: Color = Color::srgba(0.15, 0.15, 0.21, 0.95);
const ROW_BG_HOVER: Color = Color::srgba(0.26, 0.26, 0.36, 0.98);
const ROW_BORDER: Color = Color::srgba(0.30, 0.30, 0.38, 1.0);
const ROW_BORDER_SELECTED: Color = Color::srgb(1.0, 0.85, 0.20);

#[derive(Component, Reflect, Debug, Default)]
pub struct CraftingMenuRoot;

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

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnExit(GameState::Playing),
            (close_menu, despawn_crafting_menu),
        )
        .add_systems(
            Update,
            (sync_crafting_menu, refresh_crafting_menu, style_hovered_row)
                .chain()
                .after(CraftingMenuSet::Menu),
        );
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

fn row_bundle(index: usize) -> impl Bundle {
    (
        RecipeRow { index },
        Button,
        Interaction::default(),
        FocusPolicy::Block,
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(ROW_HEIGHT),
            padding: UiRect::px(8.0, 8.0, 4.0, 4.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: Val::Px(10.0),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(ROW_BG),
        BorderColor::all(ROW_BORDER),
    )
}

fn spawn_row(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    title: String,
    status: (String, Color),
    name_width: f32,
) {
    parent.spawn(row_bundle(index)).with_children(|row| {
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

fn row_title(index: usize, gear: &PlayerGear) -> String {
    match row_action(index, &gear.owned) {
        Some(RowAction::Craft(set)) => set.label().to_string(),
        Some(RowAction::Equip(piece)) => piece.name().to_string(),
        None => String::new(),
    }
}

fn recipe_status(set: GearSet, inventory: &Inventory, gear: &PlayerGear) -> (String, Color) {
    let recipe = recipe_for_set(set);
    let have = recipe
        .cost
        .iter()
        .map(|(item, required)| format!("{} {}/{}", item.label(), inventory.count(item), required))
        .collect::<Vec<String>>()
        .join(", ");
    if gear.owns_set(set) {
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
        Some(RowAction::Craft(set)) => recipe_status(set, inventory, gear),
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

fn spawn_crafting_menu(
    mut commands: Commands,
    menu: &CraftingMenu,
    inventory: &Inventory,
    gear: &PlayerGear,
) {
    commands
        .spawn((
            Name::new("Crafting Menu Root"),
            CraftingMenuRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Name::new("Crafting Panel"),
                    Node {
                        width: Val::Px(PANEL_WIDTH),
                        max_height: Val::Percent(92.0),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0),
                        padding: UiRect::all(Val::Px(16.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(PANEL_BG),
                    BorderColor::all(PANEL_BORDER),
                ))
                .with_children(|panel| {
                    panel.spawn((Name::new("Title"), label("CRAFTING", 26.0, TEXT_PRIMARY)));
                    panel.spawn((
                        Name::new("Inventory"),
                        MenuText::Line(MenuLine::Inventory),
                        label(
                            format!("Inventory:  {}", inventory.summary()),
                            14.0,
                            TEXT_DIM,
                        ),
                    ));
                    panel.spawn((
                        Name::new("Equipped"),
                        MenuText::Line(MenuLine::Equipped),
                        label(gear.equipped_summary(), 14.0, TEXT_PRIMARY),
                    ));
                    panel
                        .spawn((
                            Name::new("Columns"),
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Row,
                                column_gap: Val::Px(16.0),
                                ..default()
                            },
                        ))
                        .with_children(|columns| {
                            columns
                                .spawn((Name::new("Recipes"), column_node(60.0)))
                                .with_children(|recipes| {
                                    for index in 0..RECIPE_COUNT {
                                        spawn_row(
                                            recipes,
                                            index,
                                            row_title(index, gear),
                                            row_status(index, inventory, gear),
                                            RECIPE_NAME_WIDTH,
                                        );
                                    }
                                });
                            columns
                                .spawn((Name::new("Owned"), column_node(40.0)))
                                .with_children(|owned| {
                                    for (offset, _piece) in gear.owned.iter().enumerate() {
                                        let index = RECIPE_COUNT + offset;
                                        spawn_row(
                                            owned,
                                            index,
                                            row_title(index, gear),
                                            row_status(index, inventory, gear),
                                            OWNED_NAME_WIDTH,
                                        );
                                    }
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

fn column_node(flex_basis: f32) -> Node {
    Node {
        flex_basis: Val::Percent(flex_basis),
        flex_grow: 0.0,
        flex_shrink: 1.0,
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(6.0),
        ..default()
    }
}

fn close_menu(mut menu: ResMut<CraftingMenu>) {
    menu.close_menu();
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

fn sync_crafting_menu(
    mut commands: Commands,
    menu: Res<CraftingMenu>,
    inventory: Res<Inventory>,
    gear: Res<PlayerGear>,
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

    match action {
        MenuSync::Spawn => spawn_crafting_menu(commands, &menu, &inventory, &gear),
        MenuSync::Despawn => despawn_crafting_menu(commands, roots),
        MenuSync::Rebuild => {
            despawn_crafting_menu(commands.reborrow(), roots);
            spawn_crafting_menu(commands, &menu, &inventory, &gear);
        }
        MenuSync::Nothing => {}
    }
}

fn refresh_crafting_menu(
    menu: Res<CraftingMenu>,
    inventory: Res<Inventory>,
    gear: Res<PlayerGear>,
    mut texts: Query<(&MenuText, &mut Text, &mut TextColor)>,
    mut rows: Query<(&RecipeRow, &mut BorderColor)>,
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
    for (row, mut border) in rows.iter_mut() {
        border.set_all(if row.index == menu.selected {
            ROW_BORDER_SELECTED
        } else {
            ROW_BORDER
        });
    }
}

fn style_hovered_row(
    mut rows: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<RecipeRow>)>,
) {
    for (interaction, mut background) in rows.iter_mut() {
        background.0 = if *interaction == Interaction::Hovered {
            ROW_BG_HOVER
        } else {
            ROW_BG
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::gear::{GearSlot, MaterialType};
    use crate::components::pot::CropType;
    use bevy::state::app::StatesPlugin;
    use bevy::transform::TransformPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<CraftingMenu>()
            .init_resource::<Inventory>()
            .init_resource::<PlayerGear>()
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

    fn root_entities(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<CraftingMenuRoot>>()
            .iter(app.world())
            .collect()
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
            vec!["Wooden Sword".to_string()]
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
            assert_eq!(row_text(&mut app, index), vec![set.label().to_string()]);
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
        assert!(statuses[0].contains("Starter Crop 12/10"));
        assert!(statuses[0].contains("Craft"));
    }

    #[test]
    fn locked_recipes_are_marked_missing() {
        let mut app = setup_app();
        enter_playing(&mut app);
        open_menu(&mut app);

        let statuses = status_text(&mut app, 1);
        assert!(statuses[0].contains("Crop A 0/5"));
        assert!(statuses[0].contains("Boss A Material 0/3"));
        assert!(statuses[0].contains("Missing"));
    }

    #[test]
    fn owned_sets_are_marked_owned_instead_of_craftable() {
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
        assert!(lines[0].contains("Weapon: Wooden Sword"));
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
            vec!["Wooden Sword".to_string()]
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
        assert_eq!(notice_color("Starter Set already owned"), TEXT_BLOCKED);
        assert_eq!(notice_color("Crafted Starter Set"), TEXT_CRAFTABLE);
    }

    #[test]
    fn recipe_status_line_covers_every_state() {
        let mut inventory = Inventory::default();
        inventory.add_crop(CropType::Starter, 12);
        assert!(
            recipe_status(GearSet::Starter, &inventory, &PlayerGear::default())
                .0
                .contains("Craft")
        );

        let gear = gear_with_starter_set();
        assert!(
            recipe_status(GearSet::Starter, &inventory, &gear)
                .0
                .contains("Owned")
        );

        let (blocked, color) = recipe_status(
            GearSet::Starter,
            &Inventory::default(),
            &PlayerGear::default(),
        );
        assert!(blocked.contains("Starter Crop 0/10"));
        assert!(blocked.contains("Missing"));
        assert_eq!(color, TEXT_BLOCKED);
    }

    #[test]
    fn master_row_lists_every_cost_line() {
        let mut inventory = Inventory::default();
        inventory.add_crop(CropType::Starter, 5);
        inventory.add_material(MaterialType::BossA, 2);
        let (status, _) = recipe_status(GearSet::Master, &inventory, &PlayerGear::default());

        assert!(status.contains("Starter Crop 5/5"));
        assert!(status.contains("Crop A 0/5"));
        assert!(status.contains("Boss A Material 2/2"));
        assert!(status.contains("Boss B Material 0/2"));
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
        assert_eq!(row_title(0, &PlayerGear::default()), "Starter Set");
        assert_eq!(row_title(RECIPE_COUNT, &PlayerGear::default()), "");
    }
}
