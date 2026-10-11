//! The title screen.
//!
//! Shown once the boot assets settle. `New Game` starts a fresh run; the
//! `Difficulty` and `Load Game` slots are reserved and hidden (they keep their
//! layout room so revealing them later needs no relayout).

use crate::events::{PlaySfx, Sfx};
use crate::plugins::difficulty::DifficultySet;
use crate::plugins::ui_theme::{self as theme, ButtonSpec, MenuArt, MenuButtonSelected};
use crate::resources::boss_progress::BossProgress;
use crate::resources::boss_select::BossSelectMenu;
use crate::resources::crafting_menu::CraftingMenu;
use crate::resources::crop_select::CropSelectMenu;
use crate::resources::day_cycle::DayCycle;
use crate::resources::difficulty::Difficulty;
use crate::resources::difficulty_menu::{DifficultyMenuMode, DifficultySelectMenu};
use crate::resources::farm::{CropUnlocks, DayCounter, FarmState};
use crate::resources::goals::GoalsProgress;
use crate::resources::inventory::Inventory;
use crate::resources::inventory_panel::InventoryPanel;
use crate::resources::level::LevelRequest;
use crate::resources::main_menu::{MainMenu, MenuFeatureFlags, NewGameRequested};
use crate::resources::pause::PauseMenu;
use crate::resources::run_data::PlayerGear;
use crate::states::GameState;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// The title screen art. It has the game's name painted into it, so the screen
/// shows no title text of its own.
pub const TITLE_BACKGROUND: &str = "reference_images/title_background.png";

/// The art's width divided by its height (1600x900). The background is sized
/// from the viewport height using this, so the art always spans the screen top
/// to bottom: narrower screens crop its sides, wider ones show the backdrop as
/// pillar boxes.
const TITLE_ASPECT: f32 = 16.0 / 9.0;

/// How far down the screen (in percent) the top button sits. The art's title
/// ends just under halfway, so the menu starts well below it.
const MENU_TOP: f32 = 66.0;

/// Width of the button column, in pixels. The buttons fill it.
const MENU_WIDTH: f32 = 360.0;

/// Behind the art. Shows as the pillar boxes on screens wider than 16:9.
const BACKDROP: Color = Color::srgb(0.05, 0.04, 0.08);

#[derive(Component, Reflect, Debug, Default)]
pub struct MainMenuRoot;

/// The full-screen art behind the buttons.
#[derive(Component, Reflect, Debug, Default)]
pub struct MainMenuBackground;

/// The centred column of buttons, positioned `MENU_TOP` down the screen.
#[derive(Component, Reflect, Debug, Default)]
pub struct MainMenuColumn;

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuAction {
    NewGame,
    LoadGame,
}

#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct MainMenuButton {
    pub action: MainMenuAction,
    pub index: usize,
}

const BUTTON_NEW_GAME: usize = 0;
const BUTTON_LOAD_GAME: usize = 1;

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainMenu>()
            .init_resource::<MenuArt>()
            .init_resource::<MenuFeatureFlags>()
            .init_resource::<Difficulty>()
            .init_resource::<DifficultySelectMenu>()
            .add_message::<NewGameRequested>()
            .add_message::<crate::events::PlaySfx>()
            .add_systems(OnEnter(GameState::MainMenu), spawn_main_menu)
            .add_systems(OnExit(GameState::MainMenu), despawn_main_menu)
            .add_systems(
                Update,
                (main_menu_keyboard, main_menu_click, refresh_main_menu)
                    .chain()
                    .run_if(in_state(GameState::MainMenu))
                    .before(DifficultySet),
            )
            .add_systems(Update, on_new_game_requested);
    }
}

fn spawn_main_menu(
    mut commands: Commands,
    mut art: ResMut<MenuArt>,
    server: Option<Res<AssetServer>>,
) {
    commands
        .spawn((
            Name::new("Main Menu Root"),
            MainMenuRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                // Taller-than-16:9 screens crop the art's sides here.
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(BACKDROP),
            GlobalZIndex(400),
        ))
        .with_children(|screen| {
            // Sized from the viewport height so the art fills the screen top to
            // bottom, then pulled left by half its own width from the screen
            // centre to centre it. The root's clip hides the overhang on narrow
            // screens; on wider ones the backdrop shows either side.
            screen.spawn((
                Name::new("Title Background"),
                MainMenuBackground,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    top: Val::Px(0.0),
                    width: Val::Vh(100.0 * TITLE_ASPECT),
                    height: Val::Vh(100.0),
                    margin: UiRect::left(Val::Vh(-50.0 * TITLE_ASPECT)),
                    ..default()
                },
                ImageNode {
                    image: art.texture(TITLE_BACKGROUND, server.as_deref()),
                    image_mode: NodeImageMode::Stretch,
                    ..default()
                },
            ));

            // The buttons sit directly on the art, a centred column starting
            // `MENU_TOP` down the screen.
            screen
                .spawn((
                    Name::new("Menu Column"),
                    MainMenuColumn,
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Percent(MENU_TOP),
                        left: Val::Percent(50.0),
                        width: Val::Px(MENU_WIDTH),
                        margin: UiRect::left(Val::Px(-MENU_WIDTH / 2.0)),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(14.0),
                        ..default()
                    },
                ))
                .with_children(|column| {
                    spawn_button(
                        column,
                        &mut art,
                        server.as_deref(),
                        MainMenuAction::NewGame,
                        BUTTON_NEW_GAME,
                        "New Game",
                    );
                    // Reserved slot, hidden until Load Game lands. Choosing a
                    // difficulty lives in the New Game prompt, not here.
                    spawn_button(
                        column,
                        &mut art,
                        server.as_deref(),
                        MainMenuAction::LoadGame,
                        BUTTON_LOAD_GAME,
                        "Load Game",
                    );

                    column.spawn((
                        Name::new("Hint"),
                        Node {
                            margin: UiRect::top(Val::Px(8.0)),
                            ..default()
                        },
                        theme::label(
                            "Arrows select   |   Enter / Space start",
                            13.0,
                            theme::TEXT_DIM,
                        ),
                    ));
                });
        });
}

fn spawn_button(
    parent: &mut ChildSpawnerCommands,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
    action: MainMenuAction,
    index: usize,
    label: &str,
) {
    let skin = match action {
        MainMenuAction::NewGame => "new_game",
        MainMenuAction::LoadGame => "load_game",
    };
    let mut spec = ButtonSpec::new(index, label).label_size(22.0).skin(skin);
    // Load Game stays reserved.
    if action == MainMenuAction::LoadGame {
        spec = spec.hidden();
    }
    let visuals = art.resolve(&spec, server);
    theme::spawn_menu_button(
        parent,
        spec,
        visuals,
        MainMenuButton { action, index },
        |_| {},
    );
}

fn despawn_main_menu(mut commands: Commands, roots: Query<Entity, With<MainMenuRoot>>) {
    for entity in roots.iter() {
        commands.entity(entity).despawn();
    }
}

fn main_menu_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<MainMenu>,
    mut difficulty_menu: ResMut<DifficultySelectMenu>,
    difficulty: Res<Difficulty>,
    buttons: Query<(&MainMenuButton, &theme::MenuButton)>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if difficulty_menu.open {
        return;
    }
    let enabled: Vec<usize> = enabled_indices(buttons.iter());

    let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW);
    let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS);
    if up {
        menu.selected = theme::step_enabled(&enabled, menu.selected, -1);
    } else if down {
        menu.selected = theme::step_enabled(&enabled, menu.selected, 1);
    }

    let confirm = keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::KeyE);
    if confirm && let Some(action) = action_for_selection(menu.selected, buttons.iter()) {
        activate(action, *difficulty, &mut difficulty_menu, &mut sfx);
    }
}

fn main_menu_click(
    buttons: Query<(Entity, &MainMenuButton), Changed<Interaction>>,
    all_buttons: Query<&theme::MenuButton>,
    interactions: Query<&Interaction>,
    mut difficulty_menu: ResMut<DifficultySelectMenu>,
    difficulty: Res<Difficulty>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if difficulty_menu.open {
        return;
    }
    for (entity, button) in buttons.iter() {
        if all_buttons.get(entity).is_ok_and(|core| !core.enabled) {
            continue;
        }
        if interactions.get(entity) == Ok(&Interaction::Pressed) {
            activate(button.action, *difficulty, &mut difficulty_menu, &mut sfx);
        }
    }
}

/// Opens the picker for the action, if it is one that opens it.
fn activate(
    action: MainMenuAction,
    difficulty: Difficulty,
    menu: &mut DifficultySelectMenu,
    sfx: &mut MessageWriter<PlaySfx>,
) {
    let mode = match action {
        MainMenuAction::NewGame => DifficultyMenuMode::NewGame,
        MainMenuAction::LoadGame => return,
    };
    menu.open_menu(mode, difficulty);
    sfx.write(PlaySfx(Sfx::MenuOpen));
}

/// The sorted indices of the buttons a player can navigate to.
fn enabled_indices<'a>(
    buttons: impl Iterator<Item = (&'a MainMenuButton, &'a theme::MenuButton)>,
) -> Vec<usize> {
    let mut enabled: Vec<usize> = buttons
        .filter(|(_, core)| core.enabled)
        .map(|(button, _)| button.index)
        .collect();
    enabled.sort_unstable();
    enabled
}

/// The action of the button at `selected`, if it is enabled.
fn action_for_selection<'a>(
    selected: usize,
    buttons: impl Iterator<Item = (&'a MainMenuButton, &'a theme::MenuButton)>,
) -> Option<MainMenuAction> {
    buttons
        .filter(|(_, core)| core.enabled)
        .find(|(button, _)| button.index == selected)
        .map(|(button, _)| button.action)
}

fn refresh_main_menu(
    menu: Res<MainMenu>,
    mut buttons: Query<(&MainMenuButton, &theme::MenuButton, &mut MenuButtonSelected)>,
) {
    for (button, core, mut selected) in buttons.iter_mut() {
        selected.0 = core.enabled && button.index == menu.selected;
    }
}

/// Resets every run-scoped resource so a new game starts clean, then begins
/// play. Bundled into one param so the signature stays readable.
#[derive(SystemParam)]
pub struct ResetWork<'w> {
    inventory: ResMut<'w, Inventory>,
    gear: ResMut<'w, PlayerGear>,
    progress: ResMut<'w, BossProgress>,
    unlocks: ResMut<'w, CropUnlocks>,
    day: ResMut<'w, DayCounter>,
    cycle: ResMut<'w, DayCycle>,
    farm: ResMut<'w, FarmState>,
    crafting: ResMut<'w, CraftingMenu>,
    panel: ResMut<'w, InventoryPanel>,
    crop_select: ResMut<'w, CropSelectMenu>,
    boss_select: ResMut<'w, BossSelectMenu>,
    pause: ResMut<'w, PauseMenu>,
    goals: ResMut<'w, GoalsProgress>,
    level_request: ResMut<'w, LevelRequest>,
    next_game: ResMut<'w, NextState<GameState>>,
}

impl ResetWork<'_> {
    fn reset(&mut self) {
        *self.inventory = Inventory::default();
        *self.gear = PlayerGear::default();
        *self.progress = BossProgress::default();
        *self.unlocks = CropUnlocks::default();
        *self.day = DayCounter::default();
        *self.cycle = DayCycle::default();
        *self.farm = FarmState::default();
        *self.goals = GoalsProgress::default();
        self.crafting.close_menu();
        self.panel.close_panel();
        self.crop_select.close_menu();
        self.boss_select.close_menu();
        self.pause.close_menu();
        *self.level_request = LevelRequest::default();
    }
}

fn on_new_game_requested(
    mut requests: MessageReader<NewGameRequested>,
    mut work: ResetWork,
    mut sfx: MessageWriter<PlaySfx>,
) {
    for _ in requests.read() {
        work.reset();
        sfx.write(PlaySfx(Sfx::MenuConfirm));
        work.next_game.set(GameState::Playing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .add_plugins((
                MinimalPlugins,
                StatesPlugin,
                MainMenuPlugin,
                crate::plugins::difficulty::DifficultyPlugin,
            ))
            .init_state::<GameState>()
            .init_resource::<Inventory>()
            .init_resource::<PlayerGear>()
            .init_resource::<BossProgress>()
            .init_resource::<CropUnlocks>()
            .init_resource::<DayCounter>()
            .init_resource::<DayCycle>()
            .init_resource::<FarmState>()
            .init_resource::<GoalsProgress>()
            .init_resource::<CraftingMenu>()
            .init_resource::<InventoryPanel>()
            .init_resource::<CropSelectMenu>()
            .init_resource::<BossSelectMenu>()
            .init_resource::<PauseMenu>()
            .init_resource::<LevelRequest>();
        app
    }

    fn enter_menu(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::MainMenu);
        app.update();
        app.update();
    }

    fn roots(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<MainMenuRoot>>()
            .iter(app.world())
            .collect()
    }

    fn backgrounds(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<MainMenuBackground>>()
            .iter(app.world())
            .count()
    }

    fn buttons(app: &mut App) -> Vec<(usize, bool)> {
        let mut rows: Vec<(usize, bool)> = app
            .world_mut()
            .query_filtered::<(&MainMenuButton, &crate::plugins::ui_theme::MenuButton), With<crate::plugins::ui_theme::MenuButton>>()
            .iter(app.world())
            .map(|(action, button)| (action.index, button.enabled))
            .collect();
        rows.sort_by_key(|(index, _)| *index);
        rows
    }

    fn tap(app: &mut App, key: KeyCode) {
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

    #[test]
    fn the_menu_shows_the_art_with_a_new_game_button_on_top() {
        let mut app = setup_app();
        enter_menu(&mut app);

        assert_eq!(roots(&mut app).len(), 1);
        assert_eq!(backgrounds(&mut app), 1, "the title art backs the menu");

        let rows = buttons(&mut app);
        assert_eq!(rows.len(), 2, "new game plus one reserved slot");
        assert!(rows[0].1, "new game is enabled");
        assert!(!rows[1].1, "the load-game slot is still reserved");
    }

    #[test]
    fn the_art_preserves_its_aspect_and_fills_the_height() {
        let mut app = setup_app();
        enter_menu(&mut app);

        let node = app
            .world_mut()
            .query_filtered::<&Node, With<MainMenuBackground>>()
            .iter(app.world())
            .next()
            .expect("title background node");

        assert_eq!(node.height, Val::Vh(100.0), "the art fills the height");
        assert_eq!(node.width, Val::Vh(100.0 * TITLE_ASPECT));
        assert_eq!(
            node.left,
            Val::Percent(50.0),
            "the art is anchored at the screen centre"
        );
        assert_eq!(
            node.margin.left,
            Val::Vh(-50.0 * TITLE_ASPECT),
            "then pulled left by half its width, centring it"
        );
        assert!(TITLE_BACKGROUND.starts_with("reference_images/"));
    }

    #[test]
    fn the_button_column_starts_two_thirds_down_and_is_centred() {
        let mut app = setup_app();
        enter_menu(&mut app);

        let node = app
            .world_mut()
            .query_filtered::<&Node, With<MainMenuColumn>>()
            .iter(app.world())
            .next()
            .expect("menu column node");

        assert_eq!(node.top, Val::Percent(MENU_TOP));
        assert_eq!(MENU_TOP, 66.0);
        assert_eq!(node.left, Val::Percent(50.0));
        assert_eq!(node.margin.left, Val::Px(-MENU_WIDTH / 2.0), "centred");
    }

    #[test]
    fn the_new_game_picker_starts_on_the_current_default_difficulty() {
        let mut app = setup_app();
        enter_menu(&mut app);
        *app.world_mut().resource_mut::<Difficulty>() = Difficulty::Hard;

        tap(&mut app, KeyCode::Enter);

        let menu = app.world().resource::<DifficultySelectMenu>();
        assert!(menu.open);
        assert_eq!(menu.mode, DifficultyMenuMode::NewGame);
        assert_eq!(
            menu.selected,
            Difficulty::Hard.index(),
            "the picker opens on the last-chosen difficulty"
        );
    }

    #[test]
    fn confirming_new_game_requests_a_fresh_run_and_enters_playing() {
        let mut app = setup_app();
        enter_menu(&mut app);
        app.world_mut()
            .resource_mut::<Inventory>()
            .add_crop(crate::components::pot::CropType::Starter, 5);

        // New Game now asks for a difficulty first, then starts the run.
        tap(&mut app, KeyCode::Enter);
        assert!(
            app.world().resource::<DifficultySelectMenu>().open,
            "new game opens the difficulty picker"
        );
        assert_eq!(
            app.world().resource::<State<GameState>>().get(),
            &GameState::MainMenu,
            "the run has not started yet"
        );

        tap(&mut app, KeyCode::Enter);
        // The run begins the frame after the request is handled.
        app.update();

        assert_eq!(
            app.world().resource::<State<GameState>>().get(),
            &GameState::Playing
        );
        assert_eq!(
            app.world()
                .resource::<Inventory>()
                .crop_count(crate::components::pot::CropType::Starter),
            0,
            "a new run clears the inventory"
        );
        assert!(roots(&mut app).is_empty(), "the title screen is torn down");
    }
}
