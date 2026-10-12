//! The difficulty picker.
//!
//! It is opened from the title screen's `Difficulty` button, the title screen's
//! `New Game` (which must choose a difficulty before a run can start) and the
//! pause overlay's `Difficulty` button. The same overlay serves all three: the
//! [`DifficultyMenuMode`] decides whether confirming starts a fresh run and
//! whether `Permadeath` may be chosen.
//!
//! Locked choices (Permadeath when a run is already live, or the pause button
//! once the run *is* Permadeath) are shown greyed and flash red when pressed,
//! so the refusal is visible without a modal.

use crate::events::{PlaySfx, Sfx};
use crate::plugins::pause::{BUTTON_DIFFICULTY, PauseAction, PauseButton};
use crate::plugins::ui_theme::{self as theme, MenuButtonFlash};
use crate::resources::difficulty::Difficulty;
use crate::resources::difficulty_menu::{DifficultyMenuMode, DifficultySelectMenu};
use crate::resources::main_menu::NewGameRequested;
use crate::resources::pause::PauseMenu;
use bevy::prelude::*;

/// The difficulty overlay's root, used to spawn and tear it down.
#[derive(Component, Reflect, Debug, Default)]
pub struct DifficultyRoot;

/// A clickable difficulty row.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct DifficultyOption {
    pub index: usize,
}

/// Text on the difficulty overlay, refreshed as the highlight moves.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifficultyText {
    Title,
    RowName(usize),
    RowSummary(usize),
    Description,
}

/// All difficulty input lives in this set so the pause overlay can order its
/// own `Esc` handling before it (see `PausePlugin`).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DifficultySet;

pub struct DifficultyPlugin;

impl Plugin for DifficultyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Difficulty>()
            .init_resource::<DifficultySelectMenu>()
            .init_resource::<theme::MenuArt>()
            .init_resource::<crate::resources::confirm::ConfirmDialog>()
            .add_message::<NewGameRequested>()
            .add_message::<PlaySfx>()
            .add_systems(
                Update,
                (
                    open_from_pause,
                    difficulty_input,
                    difficulty_click,
                    clear_open_frame,
                    sync_difficulty_ui,
                    refresh_difficulty_ui,
                )
                    .chain()
                    .in_set(DifficultySet)
                    .before(theme::MenuStyleSet),
            );
    }
}

// --- Input -------------------------------------------------------------------

/// Opens the picker from the pause overlay: either the `Difficulty` button is
/// clicked, or it is highlighted and confirmed. Once the run is Permadeath the
/// button refuses and flashes instead.
fn open_from_pause(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<DifficultySelectMenu>,
    difficulty: Res<Difficulty>,
    pause: Res<PauseMenu>,
    confirm: Res<crate::resources::confirm::ConfirmDialog>,
    mut clicks: Query<(&Interaction, &PauseButton, &mut MenuButtonFlash), Changed<Interaction>>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if menu.open || !pause.open || confirm.open {
        return;
    }

    let mut clicked = false;
    for (interaction, button, mut flash) in clicks.iter_mut() {
        if button.action != PauseAction::Difficulty || *interaction != Interaction::Pressed {
            continue;
        }
        if *difficulty == Difficulty::Permadeath {
            flash.trigger();
            sfx.write(PlaySfx(Sfx::MenuClose));
        } else {
            clicked = true;
        }
    }

    if *difficulty == Difficulty::Permadeath {
        return;
    }

    let confirmed = keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::KeyE);
    let opened_by_key = confirmed && pause.selected == BUTTON_DIFFICULTY;
    if clicked || opened_by_key {
        menu.open_menu(DifficultyMenuMode::MidRun, *difficulty);
        sfx.write(PlaySfx(Sfx::MenuOpen));
    }
}

/// Keyboard navigation and confirmation for the picker.
fn difficulty_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<DifficultySelectMenu>,
    mut difficulty: ResMut<Difficulty>,
    mut requests: MessageWriter<NewGameRequested>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if !menu.open || menu.just_opened {
        return;
    }

    if theme::back_pressed(&keys) {
        menu.close_menu();
        sfx.write(PlaySfx(Sfx::MenuClose));
        return;
    }

    let enabled = menu.enabled_indices();
    let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW);
    let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS);
    if up {
        menu.selected = theme::step_enabled(&enabled, menu.selected, -1);
        sfx.write(PlaySfx(Sfx::MenuNavigate));
    } else if down {
        menu.selected = theme::step_enabled(&enabled, menu.selected, 1);
        sfx.write(PlaySfx(Sfx::MenuNavigate));
    }

    let confirm = keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::KeyE);
    if confirm {
        confirm_selection(&mut menu, &mut difficulty, &mut requests, &mut sfx);
    }
}

/// Clicking a row selects and confirms it. A locked row (Permadeath mid-run)
/// flashes instead.
fn difficulty_click(
    mut rows: Query<(&Interaction, &DifficultyOption, &mut MenuButtonFlash), Changed<Interaction>>,
    mut menu: ResMut<DifficultySelectMenu>,
    mut difficulty: ResMut<Difficulty>,
    mut requests: MessageWriter<NewGameRequested>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if !menu.open || menu.just_opened {
        return;
    }
    for (interaction, row, mut flash) in rows.iter_mut() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if !menu.is_enabled(row.index) {
            flash.trigger();
            sfx.write(PlaySfx(Sfx::MenuClose));
            continue;
        }
        menu.selected = row.index;
        confirm_selection(&mut menu, &mut difficulty, &mut requests, &mut sfx);
    }
}

/// Applies the highlighted difficulty. A `New Game` pick also starts the run.
fn confirm_selection(
    menu: &mut DifficultySelectMenu,
    difficulty: &mut Difficulty,
    requests: &mut MessageWriter<NewGameRequested>,
    sfx: &mut MessageWriter<PlaySfx>,
) {
    if !menu.is_enabled(menu.selected) {
        return;
    }
    *difficulty = menu.selected_difficulty();
    let start_run = menu.mode == DifficultyMenuMode::NewGame;
    menu.close_menu();
    sfx.write(PlaySfx(Sfx::MenuConfirm));
    if start_run {
        requests.write(NewGameRequested);
    }
}

/// Clears the opening-frame guard after the input systems have had their say.
fn clear_open_frame(mut menu: ResMut<DifficultySelectMenu>) {
    if menu.just_opened {
        menu.just_opened = false;
    }
}

// --- Overlay -----------------------------------------------------------------

fn sync_difficulty_ui(
    mut commands: Commands,
    menu: Res<DifficultySelectMenu>,
    mut art: ResMut<theme::MenuArt>,
    server: Option<Res<AssetServer>>,
    roots: Query<Entity, With<DifficultyRoot>>,
) {
    let present = !roots.is_empty();
    if menu.open && !present {
        spawn_difficulty(&mut commands, &menu, &mut art, server.as_deref());
    } else if !menu.open && present {
        for entity in roots.iter() {
            commands.entity(entity).despawn();
        }
    }
}

fn difficulty_skin(difficulty: Difficulty) -> &'static str {
    match difficulty {
        Difficulty::Easy => "difficulty_easy",
        Difficulty::Normal => "difficulty_normal",
        Difficulty::Hard => "difficulty_hard",
        Difficulty::Apocalypse => "difficulty_apocalypse",
        Difficulty::Permadeath => "difficulty_permadeath",
    }
}

fn spawn_difficulty(
    commands: &mut Commands,
    menu: &DifficultySelectMenu,
    art: &mut theme::MenuArt,
    server: Option<&AssetServer>,
) {
    commands
        .spawn((
            Name::new("Difficulty Root"),
            DifficultyRoot,
            theme::screen_root_node(),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.62)),
            GlobalZIndex(600),
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Name::new("Difficulty Panel"),
                    Node {
                        width: Val::Px(620.0),
                        max_width: Val::Percent(92.0),
                        max_height: Val::Percent(90.0),
                        min_height: Val::Px(0.0),
                        overflow: Overflow::clip(),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(10.0),
                        padding: UiRect::all(Val::Px(24.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(theme::PANEL_BG),
                    BorderColor::all(theme::PANEL_BORDER),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Name::new("Title"),
                        DifficultyText::Title,
                        theme::label("DIFFICULTY", 30.0, theme::TEXT_PRIMARY),
                    ));

                    panel
                        .spawn((
                            Name::new("Rows"),
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(6.0),
                                ..default()
                            },
                        ))
                        .with_children(|rows| {
                            for (index, difficulty) in Difficulty::ALL.iter().enumerate() {
                                spawn_difficulty_row(rows, index, *difficulty, menu, art, server);
                            }
                        });

                    panel.spawn((
                        Name::new("Description"),
                        DifficultyText::Description,
                        theme::label(
                            menu.selected_difficulty().description(),
                            14.0,
                            theme::TEXT_DIM,
                        ),
                    ));
                    panel.spawn((
                        Name::new("Hint"),
                        theme::label(
                            "Arrows select   |   Enter / click chooses   |   Esc / Tab cancels",
                            13.0,
                            theme::TEXT_DIM,
                        ),
                    ));
                });
        });
}

fn spawn_difficulty_row(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    difficulty: Difficulty,
    menu: &DifficultySelectMenu,
    art: &mut theme::MenuArt,
    server: Option<&AssetServer>,
) {
    let enabled = menu.is_enabled(index);
    let spec = theme::ButtonSpec::new(index, difficulty.label()).skin(difficulty_skin(difficulty));
    let visuals = art.resolve(&spec, server);
    let name_color = if enabled {
        theme::TEXT_PRIMARY
    } else {
        theme::TEXT_DIM
    };
    parent
        .spawn((
            theme::button_core(index, enabled, false, visuals),
            DifficultyOption { index },
            MenuButtonFlash::default(),
        ))
        .with_children(|row| {
            row.spawn((
                DifficultyText::RowName(index),
                theme::label(difficulty.label(), 20.0, name_color),
                Node {
                    width: Val::Px(190.0),
                    flex_shrink: 0.0,
                    ..default()
                },
            ));
            let summary = if enabled {
                difficulty.summary()
            } else {
                "Unavailable mid-run".to_string()
            };
            row.spawn((
                DifficultyText::RowSummary(index),
                theme::label(summary, 13.0, theme::TEXT_DIM),
                TextLayout::justify(Justify::Right),
                Node {
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    ..default()
                },
            ));
        });
}

fn refresh_difficulty_ui(
    menu: Res<DifficultySelectMenu>,
    mut rows: Query<(
        &DifficultyOption,
        &mut theme::MenuButton,
        &mut theme::MenuButtonSelected,
    )>,
    mut texts: Query<(&DifficultyText, &mut Text, &mut TextColor)>,
) {
    for (row, mut button, mut selected) in rows.iter_mut() {
        let enabled = menu.open && menu.is_enabled(row.index);
        button.enabled = enabled;
        selected.0 = enabled && row.index == menu.selected;
    }

    for (marker, mut text, mut color) in texts.iter_mut() {
        match marker {
            DifficultyText::Title => {}
            DifficultyText::RowName(index) => {
                if let Some(difficulty) = Difficulty::ALL.get(*index) {
                    *text = Text::new(difficulty.label());
                    color.0 = if menu.is_enabled(*index) {
                        theme::TEXT_PRIMARY
                    } else {
                        theme::TEXT_DIM
                    };
                }
            }
            DifficultyText::RowSummary(index) => {
                if let Some(difficulty) = Difficulty::ALL.get(*index) {
                    let summary = if menu.is_enabled(*index) {
                        difficulty.summary()
                    } else {
                        "Unavailable mid-run".to_string()
                    };
                    *text = Text::new(summary);
                    color.0 = theme::TEXT_DIM;
                }
            }
            DifficultyText::Description => {
                *text = Text::new(menu.selected_difficulty().description());
                color.0 = theme::TEXT_DIM;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::pause::PauseMenu;
    use bevy::state::app::StatesPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<PauseMenu>()
            .init_resource::<Difficulty>()
            .init_resource::<DifficultySelectMenu>()
            .add_plugins((MinimalPlugins, StatesPlugin, DifficultyPlugin));
        app
    }

    fn menu(app: &App) -> &DifficultySelectMenu {
        app.world().resource::<DifficultySelectMenu>()
    }

    fn difficulty(app: &App) -> Difficulty {
        *app.world().resource::<Difficulty>()
    }

    fn roots(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<DifficultyRoot>>()
            .iter(app.world())
            .count()
    }

    fn open(app: &mut App, mode: DifficultyMenuMode) {
        let current = difficulty(app);
        app.world_mut()
            .resource_mut::<DifficultySelectMenu>()
            .open_menu(mode, current);
        app.update();
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
    fn opening_spawns_the_overlay_and_closing_drops_it() {
        let mut app = setup_app();
        assert_eq!(roots(&mut app), 0);

        open(&mut app, DifficultyMenuMode::MidRun);
        assert_eq!(roots(&mut app), 1);

        app.world_mut()
            .resource_mut::<DifficultySelectMenu>()
            .close_menu();
        app.update();
        assert_eq!(roots(&mut app), 0);
    }

    #[test]
    fn a_default_pick_updates_the_difficulty_without_starting_a_run() {
        let mut app = setup_app();
        open(&mut app, DifficultyMenuMode::Default);
        // Move down to Normal (the default is already Normal, so go to Hard).
        tap(&mut app, KeyCode::ArrowDown);
        tap(&mut app, KeyCode::Enter);

        assert_eq!(difficulty(&app), Difficulty::Hard);
        assert!(!menu(&app).open);
        assert_eq!(roots(&mut app), 0);
    }

    #[test]
    fn a_new_game_pick_starts_a_run() {
        let mut app = setup_app();
        open(&mut app, DifficultyMenuMode::NewGame);
        tap(&mut app, KeyCode::ArrowDown);

        tap(&mut app, KeyCode::Enter);

        assert_eq!(difficulty(&app), Difficulty::Hard);
        assert!(!menu(&app).open);
    }

    #[test]
    fn navigation_skips_the_locked_permadeath_row_mid_run() {
        let mut app = setup_app();
        open(&mut app, DifficultyMenuMode::MidRun);
        let last = Difficulty::ALL.len() - 2; // Apocalypse, since Permadeath is locked
        for _ in 0..Difficulty::ALL.len() {
            tap(&mut app, KeyCode::ArrowDown);
        }
        assert_eq!(menu(&app).selected, last);
    }

    #[test]
    fn cancelling_leaves_the_difficulty_untouched() {
        let mut app = setup_app();
        open(&mut app, DifficultyMenuMode::Default);
        tap(&mut app, KeyCode::ArrowDown);
        tap(&mut app, KeyCode::Escape);

        assert_eq!(difficulty(&app), Difficulty::Normal);
        assert!(!menu(&app).open);
    }

    #[test]
    fn the_opening_key_does_not_also_confirm() {
        let mut app = setup_app();
        // Simulate the opener running this frame: set just_opened, then feed
        // Enter, which must not confirm.
        app.world_mut()
            .resource_mut::<DifficultySelectMenu>()
            .open_menu(DifficultyMenuMode::NewGame, Difficulty::Normal);
        {
            let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            input.press(KeyCode::Enter);
        }
        app.update();

        assert!(menu(&app).open, "the opening key is ignored");
        assert_eq!(difficulty(&app), Difficulty::Normal);
    }

    fn pause_button(app: &mut App, action: PauseAction) -> Entity {
        app.world_mut()
            .spawn((
                PauseButton {
                    action,
                    index: BUTTON_DIFFICULTY,
                },
                MenuButtonFlash::default(),
                Interaction::None,
            ))
            .id()
    }

    #[test]
    fn the_pause_difficulty_button_opens_the_picker_mid_run() {
        let mut app = setup_app();
        {
            let mut pause = app.world_mut().resource_mut::<PauseMenu>();
            pause.open_menu();
            pause.selected = BUTTON_DIFFICULTY;
        }
        let button = pause_button(&mut app, PauseAction::Difficulty);
        app.update();
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::Pressed;
        app.update();

        assert!(menu(&app).open);
        assert_eq!(menu(&app).mode, DifficultyMenuMode::MidRun);
    }

    #[test]
    fn the_pause_button_refuses_when_the_run_is_permadeath() {
        let mut app = setup_app();
        *app.world_mut().resource_mut::<Difficulty>() = Difficulty::Permadeath;
        {
            let mut pause = app.world_mut().resource_mut::<PauseMenu>();
            pause.open_menu();
            pause.selected = BUTTON_DIFFICULTY;
        }
        let button = pause_button(&mut app, PauseAction::Difficulty);
        app.update();
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::Pressed;
        app.update();

        assert!(!menu(&app).open, "permadeath cannot be changed mid-run");
        assert!(
            app.world()
                .get::<MenuButtonFlash>(button)
                .unwrap()
                .remaining
                > 0.0,
            "the button flashes its refusal"
        );
    }
}
