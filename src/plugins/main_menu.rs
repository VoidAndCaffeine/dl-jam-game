//! The title screen.
//!
//! Shown once the boot assets settle. `New Game` starts a fresh run; the
//! `Difficulty` and `Load Game` slots are reserved and hidden (they keep their
//! layout room so revealing them later needs no relayout).

use crate::events::{PlaySfx, Sfx};
use crate::plugins::ui_theme::{self as theme, ButtonSpec, MenuArt, MenuButtonSelected};
use crate::resources::boss_progress::BossProgress;
use crate::resources::boss_select::BossSelectMenu;
use crate::resources::crafting_menu::CraftingMenu;
use crate::resources::crop_select::CropSelectMenu;
use crate::resources::day_cycle::DayCycle;
use crate::resources::farm::{CropUnlocks, DayCounter, FarmState};
use crate::resources::inventory::Inventory;
use crate::resources::inventory_panel::InventoryPanel;
use crate::resources::level::LevelRequest;
use crate::resources::main_menu::{MainMenu, MenuFeatureFlags, NewGameRequested};
use crate::resources::pause::PauseMenu;
use crate::resources::run_data::PlayerGear;
use crate::states::GameState;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// The game's title, rendered as text for now. A `MainMenuTitle` image slot is
/// reserved for a later PNG.
pub const GAME_TITLE: &str = "Vines of Ore and Mercury";

#[derive(Component, Reflect, Debug, Default)]
pub struct MainMenuRoot;

#[derive(Component, Reflect, Debug, Default)]
pub struct MainMenuTitle;

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuAction {
    NewGame,
    Difficulty,
    LoadGame,
}

#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct MainMenuButton {
    pub action: MainMenuAction,
    pub index: usize,
}

const BUTTON_NEW_GAME: usize = 0;
const BUTTON_DIFFICULTY: usize = 1;
const BUTTON_LOAD_GAME: usize = 2;

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MainMenu>()
            .init_resource::<MenuArt>()
            .init_resource::<MenuFeatureFlags>()
            .add_message::<NewGameRequested>()
            .add_message::<crate::events::PlaySfx>()
            .add_systems(OnEnter(GameState::MainMenu), spawn_main_menu)
            .add_systems(OnExit(GameState::MainMenu), despawn_main_menu)
            .add_systems(
                Update,
                (main_menu_keyboard, main_menu_click, refresh_main_menu)
                    .chain()
                    .run_if(in_state(GameState::MainMenu)),
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
            theme::screen_root_node(),
            BackgroundColor(Color::srgb(0.05, 0.04, 0.08)),
            GlobalZIndex(400),
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Name::new("Title Panel"),
                    Node {
                        width: Val::Px(620.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(14.0),
                        padding: UiRect::all(Val::Px(26.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(theme::PANEL_BG),
                    BorderColor::all(theme::PANEL_BORDER),
                ))
                .with_children(|panel| {
                    // Title text now; swap in a `MainMenuTitle` image later.
                    panel.spawn((
                        Name::new("Title"),
                        MainMenuTitle,
                        theme::label(GAME_TITLE, 40.0, theme::TEXT_PRIMARY),
                        Node {
                            margin: UiRect::bottom(Val::Px(12.0)),
                            ..default()
                        },
                    ));

                    spawn_button(
                        panel,
                        &mut art,
                        server.as_deref(),
                        MainMenuAction::NewGame,
                        BUTTON_NEW_GAME,
                        "New Game",
                    );
                    // Reserved slots: hidden until the features land.
                    spawn_button(
                        panel,
                        &mut art,
                        server.as_deref(),
                        MainMenuAction::Difficulty,
                        BUTTON_DIFFICULTY,
                        "Difficulty",
                    );
                    spawn_button(
                        panel,
                        &mut art,
                        server.as_deref(),
                        MainMenuAction::LoadGame,
                        BUTTON_LOAD_GAME,
                        "Load Game",
                    );

                    panel.spawn((
                        Name::new("Hint"),
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
        MainMenuAction::Difficulty => "difficulty",
        MainMenuAction::LoadGame => "load_game",
    };
    let mut spec = ButtonSpec::new(index, label).label_size(22.0).skin(skin);
    if action != MainMenuAction::NewGame {
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
    mut requests: MessageWriter<NewGameRequested>,
) {
    let confirm = keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::KeyE);
    if confirm {
        requests.write(NewGameRequested);
    }
}

fn main_menu_click(
    buttons: Query<(Entity, &MainMenuButton), Changed<Interaction>>,
    interactions: Query<&Interaction>,
    mut requests: MessageWriter<NewGameRequested>,
) {
    for (entity, button) in buttons.iter() {
        if button.action != MainMenuAction::NewGame {
            continue;
        }
        if interactions.get(entity) == Ok(&Interaction::Pressed) {
            requests.write(NewGameRequested);
        }
    }
}

fn refresh_main_menu(mut buttons: Query<(&MainMenuButton, &mut MenuButtonSelected)>) {
    for (button, mut selected) in buttons.iter_mut() {
        selected.0 = button.action == MainMenuAction::NewGame;
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
            .add_plugins((MinimalPlugins, StatesPlugin, MainMenuPlugin))
            .init_state::<GameState>()
            .init_resource::<Inventory>()
            .init_resource::<PlayerGear>()
            .init_resource::<BossProgress>()
            .init_resource::<CropUnlocks>()
            .init_resource::<DayCounter>()
            .init_resource::<DayCycle>()
            .init_resource::<FarmState>()
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
    fn the_menu_spawns_with_a_title_and_a_new_game_button() {
        let mut app = setup_app();
        enter_menu(&mut app);

        assert_eq!(roots(&mut app).len(), 1);
        let title = app
            .world_mut()
            .query_filtered::<&Text, With<MainMenuTitle>>()
            .iter(app.world())
            .next()
            .map(|text| text.0.clone())
            .expect("title text");
        assert_eq!(title, GAME_TITLE);

        let rows = buttons(&mut app);
        assert_eq!(rows.len(), 3, "new game plus two reserved slots");
        assert!(rows[0].1, "new game is enabled");
        assert!(!rows[1].1 && !rows[2].1, "reserved slots are disabled");
    }

    #[test]
    fn confirming_new_game_requests_a_fresh_run_and_enters_playing() {
        let mut app = setup_app();
        enter_menu(&mut app);
        app.world_mut()
            .resource_mut::<Inventory>()
            .add_crop(crate::components::pot::CropType::Starter, 5);

        tap(&mut app, KeyCode::Enter);

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
