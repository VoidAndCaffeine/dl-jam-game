//! The pause overlay.
//!
//! Pause sits on top of whatever `Playing` phase is live. Opening it pauses
//! virtual time (`Time<Virtual>`) so bosses, timers and animations all stop
//! together, and resumes it on the way out. `Resume` is the first button and
//! the one selected on open; `Return to Main Menu` sits below it and a
//! `Difficulty` slot is reserved (hidden) for later.

use crate::plugins::difficulty::DifficultySet;
use crate::plugins::ui_theme::{
    self as theme, ButtonSpec, MenuArt, MenuButtonFlash, MenuButtonSelected,
};
use crate::resources::difficulty::Difficulty;
use crate::resources::difficulty_menu::DifficultySelectMenu;
use crate::resources::main_menu::MenuFeatureFlags;
use crate::resources::pause::PauseMenu;
use crate::states::{GameState, Phase};
use bevy::prelude::*;
use bevy::time::Virtual;

#[derive(Component, Reflect, Debug, Default)]
pub struct PauseRoot;

#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseAction {
    Resume,
    ReturnToMainMenu,
    Difficulty,
}

#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct PauseButton {
    pub action: PauseAction,
    pub index: usize,
}

#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct PauseHint;

const BUTTON_RESUME: usize = 0;
const BUTTON_MAIN_MENU: usize = 1;
pub const BUTTON_DIFFICULTY: usize = 2;

pub struct PausePlugin;

impl Plugin for PausePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PauseMenu>()
            .init_resource::<MenuArt>()
            .init_resource::<MenuFeatureFlags>()
            .init_resource::<Difficulty>()
            .init_resource::<DifficultySelectMenu>()
            .add_systems(OnEnter(GameState::Playing), ensure_unpaused)
            .add_systems(OnExit(GameState::Playing), (leave_pause, despawn_pause_ui))
            // `Esc` is read by every panel, so pause claims it first: a panel
            // that is already open keeps the press, and pause only opens when
            // nothing else does. The difficulty picker is ordered after pause so
            // the `Esc`/confirm that opens or closes it never also reaches pause.
            .add_systems(
                Update,
                toggle_pause
                    .before(crate::systems::crafting::CraftingMenuSet::Menu)
                    .before(crate::systems::inventory::InventoryPanelSet::Panel)
                    .before(crate::systems::crop_select::CropSelectSet::Menu)
                    .before(crate::systems::boss_select::BossSelectSet::Menu)
                    .before(DifficultySet),
            )
            .add_systems(
                Update,
                (
                    pause_keyboard,
                    pause_button_on_click,
                    drive_virtual_time,
                    sync_pause_ui,
                    refresh_pause_ui,
                )
                    .chain()
                    .before(DifficultySet),
            );
    }
}

/// The pause menu only reacts while playing and not mid-load.
fn pause_allowed(phase: &Phase) -> bool {
    phase.is_playing() && !phase.is_loading()
}

/// Every other open panel, bundled so `toggle_pause` can check them without a
/// 7-argument signature. Kept separate from `PauseMenu`, which is borrowed
/// mutably.
#[derive(bevy::ecs::system::SystemParam)]
struct OtherPanels<'w> {
    menu: Res<'w, crate::resources::crafting_menu::CraftingMenu>,
    inventory: Res<'w, crate::resources::inventory_panel::InventoryPanel>,
    crop_select: Res<'w, crate::resources::crop_select::CropSelectMenu>,
    boss_select: Res<'w, crate::resources::boss_select::BossSelectMenu>,
    difficulty: Res<'w, DifficultySelectMenu>,
}

/// `Esc` opens the pause menu when nothing else is open, and closes it when it
/// is. Panels keep their own `Esc` handling for when they are the open one.
///
/// Reads the panel resources directly rather than through `OpenPanels`, because
/// `OpenPanels` itself borrows `PauseMenu` and a system may not hold the same
/// resource both ways.
fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    mut pause: ResMut<PauseMenu>,
    panels: OtherPanels,
    phase: Phase,
) {
    if !pause_allowed(&phase) || !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    // The difficulty picker owns `Esc` while it is open.
    if panels.difficulty.open {
        return;
    }
    if pause.open {
        pause.close_menu();
    } else if !(panels.menu.open
        || panels.inventory.open
        || panels.crop_select.open
        || panels.boss_select.open)
    {
        pause.open_menu();
    }
}

/// Keyboard navigation and activation for the pause buttons.
fn pause_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut pause: ResMut<PauseMenu>,
    difficulty_menu: Res<DifficultySelectMenu>,
    mut next_game: ResMut<NextState<GameState>>,
    buttons: Query<(&PauseButton, &theme::MenuButton)>,
    phase: Phase,
) {
    if !pause.open || !pause_allowed(&phase) || difficulty_menu.open {
        return;
    }

    let enabled: Vec<usize> = {
        let mut list: Vec<usize> = buttons
            .iter()
            .filter(|(_, core)| core.enabled)
            .map(|(button, _)| button.index)
            .collect();
        list.sort_unstable();
        list
    };

    let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW);
    let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS);
    if up {
        pause.selected = theme::step_enabled(&enabled, pause.selected, -1);
    } else if down {
        pause.selected = theme::step_enabled(&enabled, pause.selected, 1);
    }

    let confirm = keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::KeyE);
    if !confirm {
        return;
    }
    match pause.selected {
        BUTTON_RESUME => pause.close_menu(),
        BUTTON_MAIN_MENU => {
            pause.close_menu();
            next_game.set(GameState::MainMenu);
        }
        // The difficulty picker watches for this itself (`open_from_pause`), so
        // the button opens it even though pause does not act on the confirm.
        _ => {}
    }
}

fn pause_button_on_click(
    buttons: Query<(Entity, &PauseButton), Changed<Interaction>>,
    interactions: Query<&Interaction>,
    mut pause: ResMut<PauseMenu>,
    mut next_game: ResMut<NextState<GameState>>,
    difficulty_menu: Res<DifficultySelectMenu>,
    phase: Phase,
) {
    if !pause.open || !pause_allowed(&phase) || difficulty_menu.open {
        return;
    }
    for (entity, button) in buttons.iter() {
        if interactions.get(entity) != Ok(&Interaction::Pressed) {
            continue;
        }
        match button.action {
            PauseAction::Resume => pause.close_menu(),
            PauseAction::ReturnToMainMenu => {
                pause.close_menu();
                next_game.set(GameState::MainMenu);
            }
            // The difficulty picker handles its own button (`open_from_pause`).
            PauseAction::Difficulty => {}
        }
    }
}

/// Pauses or resumes virtual time whenever the overlay crosses its open state.
fn drive_virtual_time(pause: Res<PauseMenu>, mut time: ResMut<Time<Virtual>>) {
    if !pause.is_changed() {
        return;
    }
    if pause.open && !time.is_paused() {
        time.pause();
    } else if !pause.open && time.is_paused() {
        time.unpause();
    }
}

fn sync_pause_ui(
    mut commands: Commands,
    pause: Res<PauseMenu>,
    difficulty: Res<Difficulty>,
    mut art: ResMut<MenuArt>,
    server: Option<Res<AssetServer>>,
    roots: Query<Entity, With<PauseRoot>>,
) {
    let present = !roots.is_empty();
    if pause.open && !present {
        spawn_pause(&mut commands, &mut art, *difficulty, server.as_deref());
    } else if !pause.open && present {
        for entity in roots.iter() {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_pause(
    commands: &mut Commands,
    art: &mut MenuArt,
    current: Difficulty,
    server: Option<&AssetServer>,
) {
    commands
        .spawn((
            Name::new("Pause Root"),
            PauseRoot,
            theme::screen_root_node(),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(500),
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Name::new("Pause Panel"),
                    Node {
                        width: Val::Px(480.0),
                        max_width: Val::Percent(92.0),
                        max_height: Val::Percent(88.0),
                        min_height: Val::Px(0.0),
                        overflow: Overflow::clip(),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(10.0),
                        padding: UiRect::all(Val::Px(22.0)),
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
                        theme::label("PAUSED", 30.0, theme::TEXT_PRIMARY),
                    ));

                    spawn_button(
                        panel,
                        art,
                        server,
                        current,
                        PauseAction::Resume,
                        BUTTON_RESUME,
                        "Resume",
                    );
                    spawn_button(
                        panel,
                        art,
                        server,
                        current,
                        PauseAction::ReturnToMainMenu,
                        BUTTON_MAIN_MENU,
                        "Return to Main Menu",
                    );
                    // Live now, but locked once the run itself is Permadeath.
                    spawn_button(
                        panel,
                        art,
                        server,
                        current,
                        PauseAction::Difficulty,
                        BUTTON_DIFFICULTY,
                        "Difficulty",
                    );

                    panel.spawn((
                        Name::new("Hint"),
                        PauseHint,
                        theme::label(
                            "Arrows select   |   Enter / Space confirm   |   Esc resumes",
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
    current: Difficulty,
    action: PauseAction,
    index: usize,
    label: &str,
) {
    let skin = match action {
        PauseAction::Resume => "resume",
        PauseAction::ReturnToMainMenu => "return_to_main_menu",
        PauseAction::Difficulty => "difficulty",
    };
    let mut spec = ButtonSpec::new(index, label).label_size(20.0).skin(skin);
    if action == PauseAction::Difficulty && current == Difficulty::Permadeath {
        // Permadeath cannot be changed once the run is under way.
        spec = spec.disabled();
    }
    let visuals = art.resolve(&spec, server);
    let marker = PauseButton { action, index };
    // The difficulty button always carries the flash component so the picker
    // can watch its clicks; it only lights red on a Permadeath refusal.
    if action == PauseAction::Difficulty {
        theme::spawn_menu_button(
            parent,
            spec,
            visuals,
            (marker, MenuButtonFlash::default()),
            |_| {},
        );
    } else {
        theme::spawn_menu_button(parent, spec, visuals, marker, |_| {});
    }
}

fn refresh_pause_ui(
    pause: Res<PauseMenu>,
    difficulty: Res<Difficulty>,
    mut buttons: Query<(
        &PauseButton,
        &mut theme::MenuButton,
        &mut MenuButtonSelected,
    )>,
) {
    for (button, mut core, mut selected) in buttons.iter_mut() {
        let enabled =
            !(button.action == PauseAction::Difficulty && *difficulty == Difficulty::Permadeath);
        core.enabled = enabled;
        selected.0 = pause.open && enabled && button.index == pause.selected;
    }
}

/// Refuses to leave virtual time paused behind.
fn ensure_unpaused(mut time: ResMut<Time<Virtual>>) {
    if time.is_paused() {
        time.unpause();
    }
}

/// Leaving play for the title screen or victory always resumes the clock and
/// drops the overlay.
fn leave_pause(mut time: ResMut<Time<Virtual>>, mut pause: ResMut<PauseMenu>) {
    if time.is_paused() {
        time.unpause();
    }
    pause.close_menu();
}

/// Tears the overlay down when play ends, however it ends.
fn despawn_pause_ui(mut commands: Commands, roots: Query<Entity, With<PauseRoot>>) {
    for entity in roots.iter() {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::crafting_menu::CraftingMenu;
    use crate::resources::crop_select::CropSelectMenu;
    use crate::resources::inventory_panel::InventoryPanel;
    use bevy::state::app::StatesPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<CraftingMenu>()
            .init_resource::<InventoryPanel>()
            .init_resource::<CropSelectMenu>()
            .init_resource::<crate::resources::boss_select::BossSelectMenu>()
            .add_plugins((MinimalPlugins, StatesPlugin, PausePlugin))
            .init_state::<GameState>()
            .init_state::<crate::states::DayPhase>();
        app
    }

    fn start_playing(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
    }

    fn pause(app: &App) -> &PauseMenu {
        app.world().resource::<PauseMenu>()
    }

    fn game_state(app: &App) -> GameState {
        app.world().resource::<State<GameState>>().get().clone()
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

    fn roots(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<PauseRoot>>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn escape_opens_and_closes_the_pause_menu() {
        let mut app = setup_app();
        start_playing(&mut app);
        assert!(!pause(&app).open);

        tap(&mut app, KeyCode::Escape);
        assert!(pause(&app).open);
        assert_eq!(roots(&mut app), 1);
        assert_eq!(pause(&app).selected, 0, "resume is auto-selected");

        tap(&mut app, KeyCode::Escape);
        assert!(!pause(&app).open);
        assert_eq!(roots(&mut app), 0);
    }

    #[test]
    fn opening_pause_freezes_virtual_time_and_resuming_thaws_it() {
        let mut app = setup_app();
        start_playing(&mut app);

        tap(&mut app, KeyCode::Escape);
        assert!(app.world().resource::<Time<Virtual>>().is_paused());

        tap(&mut app, KeyCode::Enter);
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    }

    #[test]
    fn escape_does_not_open_pause_while_another_panel_is_open() {
        let mut app = setup_app();
        start_playing(&mut app);
        app.world_mut().resource_mut::<CraftingMenu>().open_menu();

        tap(&mut app, KeyCode::Escape);

        assert!(!pause(&app).open, "the crafting menu owns this Esc");
    }

    #[test]
    fn escape_does_not_open_pause_over_boss_select() {
        let mut app = setup_app();
        start_playing(&mut app);
        app.world_mut()
            .resource_mut::<crate::resources::boss_select::BossSelectMenu>()
            .open_menu();

        tap(&mut app, KeyCode::Escape);

        assert!(!pause(&app).open, "boss select owns this Esc");
    }

    #[test]
    fn return_to_main_menu_leaves_play() {
        let mut app = setup_app();
        start_playing(&mut app);
        tap(&mut app, KeyCode::Escape);
        app.world_mut().resource_mut::<PauseMenu>().selected = 1;

        tap(&mut app, KeyCode::Enter);

        assert_eq!(game_state(&app), GameState::MainMenu);
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    }

    #[test]
    fn navigation_walks_the_enabled_buttons() {
        let mut app = setup_app();
        start_playing(&mut app);
        tap(&mut app, KeyCode::Escape);

        tap(&mut app, KeyCode::ArrowDown);
        assert_eq!(pause(&app).selected, 1);
        tap(&mut app, KeyCode::ArrowDown);
        assert_eq!(pause(&app).selected, 2, "difficulty is reachable now");
        tap(&mut app, KeyCode::ArrowDown);
        assert_eq!(pause(&app).selected, 2, "clamps at the last button");
        tap(&mut app, KeyCode::ArrowUp);
        assert_eq!(pause(&app).selected, 1);
        tap(&mut app, KeyCode::ArrowUp);
        assert_eq!(pause(&app).selected, 0);
    }

    #[test]
    fn leaving_playing_drops_the_overlay_and_resumes_time() {
        let mut app = setup_app();
        start_playing(&mut app);
        tap(&mut app, KeyCode::Escape);
        assert!(pause(&app).open);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();
        app.update();

        assert!(!pause(&app).open);
        assert!(!app.world().resource::<Time<Virtual>>().is_paused());
    }
}
