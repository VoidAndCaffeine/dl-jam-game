//! The pause overlay.
//!
//! Pause sits on top of whatever `Playing` phase is live. Opening it pauses
//! virtual time (`Time<Virtual>`) so bosses, timers and animations all stop
//! together, and resumes it on the way out. `Resume` is the first button and
//! the one selected on open; `Return to Main Menu` sits below it and a
//! `Difficulty` slot is reserved (hidden) for later.

use crate::plugins::ui_theme::{self as theme, ButtonSpec, MenuArt, MenuButtonSelected};
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
            .add_systems(OnEnter(GameState::Playing), ensure_unpaused)
            .add_systems(OnExit(GameState::Playing), (leave_pause, despawn_pause_ui))
            // `Esc` is read by every panel, so pause claims it first: a panel
            // that is already open keeps the press, and pause only opens when
            // nothing else does.
            .add_systems(
                Update,
                toggle_pause
                    .before(crate::systems::crafting::CraftingMenuSet::Menu)
                    .before(crate::systems::inventory::InventoryPanelSet::Panel)
                    .before(crate::systems::crop_select::CropSelectSet::Menu)
                    .before(crate::systems::boss_select::BossSelectSet::Menu),
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
                    .chain(),
            );
    }
}

/// The pause menu only reacts while playing and not mid-load.
fn pause_allowed(phase: &Phase) -> bool {
    phase.is_playing() && !phase.is_loading()
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
    menu: Res<crate::resources::crafting_menu::CraftingMenu>,
    inventory: Res<crate::resources::inventory_panel::InventoryPanel>,
    crop_select: Res<crate::resources::crop_select::CropSelectMenu>,
    boss_select: Res<crate::resources::boss_select::BossSelectMenu>,
    phase: Phase,
) {
    if !pause_allowed(&phase) || !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if pause.open {
        pause.close_menu();
    } else if !(menu.open || inventory.open || crop_select.open || boss_select.open) {
        pause.open_menu();
    }
}

/// Keyboard navigation and activation for the pause buttons.
fn pause_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut pause: ResMut<PauseMenu>,
    mut next_game: ResMut<NextState<GameState>>,
    phase: Phase,
) {
    if !pause.open || !pause_allowed(&phase) {
        return;
    }

    let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW);
    let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS);
    if up || down {
        pause.selected = step(pause.selected, if down { 1 } else { -1 });
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
        _ => {}
    }
}

/// Moves the highlight between the two enabled buttons.
fn step(current: usize, direction: i32) -> usize {
    let last = BUTTON_MAIN_MENU as i32;
    let target = current as i32 + direction;
    if (0..=last).contains(&target) {
        target as usize
    } else {
        current
    }
}

fn pause_button_on_click(
    buttons: Query<(Entity, &PauseButton), Changed<Interaction>>,
    interactions: Query<&Interaction>,
    mut pause: ResMut<PauseMenu>,
    mut next_game: ResMut<NextState<GameState>>,
    phase: Phase,
) {
    if !pause.open || !pause_allowed(&phase) {
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
    mut art: ResMut<MenuArt>,
    server: Option<Res<AssetServer>>,
    roots: Query<Entity, With<PauseRoot>>,
) {
    let present = !roots.is_empty();
    if pause.open && !present {
        spawn_pause(&mut commands, &mut art, server.as_deref());
    } else if !pause.open && present {
        for entity in roots.iter() {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_pause(commands: &mut Commands, art: &mut MenuArt, server: Option<&AssetServer>) {
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
                        PauseAction::Resume,
                        BUTTON_RESUME,
                        "Resume",
                    );
                    spawn_button(
                        panel,
                        art,
                        server,
                        PauseAction::ReturnToMainMenu,
                        BUTTON_MAIN_MENU,
                        "Return to Main Menu",
                    );
                    // Reserved slot: hidden until the difficulty feature lands.
                    spawn_button(
                        panel,
                        art,
                        server,
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
    if action == PauseAction::Difficulty {
        spec = spec.hidden();
    }
    let visuals = art.resolve(&spec, server);
    theme::spawn_menu_button(parent, spec, visuals, PauseButton { action, index }, |_| {});
}

fn refresh_pause_ui(
    pause: Res<PauseMenu>,
    mut buttons: Query<(&PauseButton, &mut MenuButtonSelected)>,
) {
    for (button, mut selected) in buttons.iter_mut() {
        selected.0 = pause.open
            && button.action != PauseAction::Difficulty
            && button.index == pause.selected;
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
    fn navigation_stays_on_the_two_enabled_buttons() {
        let mut app = setup_app();
        start_playing(&mut app);
        tap(&mut app, KeyCode::Escape);

        tap(&mut app, KeyCode::ArrowDown);
        assert_eq!(pause(&app).selected, 1);
        tap(&mut app, KeyCode::ArrowDown);
        assert_eq!(pause(&app).selected, 1, "the reserved slot is skipped");
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
