//! The settings overlay.
//!
//! One overlay serves both the title screen and the pause menu: it reads the
//! shared [`Settings`] resource and writes it back, so there is no per-menu
//! variant. It is split into tabs (Audio, Accessibility, Display, Controls).
//! Rows are steppers (a `-` / value / `+` cluster); the keyboard walks them with
//! Up/Down and adjusts the highlighted one with Left/Right, while the mouse
//! drives the `-` / `+` buttons directly. The Controls tab binds a primary and a
//! secondary key per action, with a reset entry.
//!
//! The settings themselves live in their own save slot and are loaded at boot
//! (see [`crate::resources::settings`]); this plugin also fans them out to the
//! systems that consume them: the audio mix, reduced-motion gating, the
//! colourblind palette and the UI text scale.

use crate::events::{PlaySfx, Sfx};
use crate::plugins::pause::{BUTTON_SETTINGS, PauseAction, PauseButton};
use crate::plugins::ui_theme::{
    self as theme, ButtonSpec, MenuArt, MenuButton, MenuButtonSelected,
};
use crate::resources::audio_settings::AudioSettings;
use crate::resources::save_manager::SaveManager;
use crate::resources::settings::{Action, ColorblindMode, KeySlot, SettingRow, Settings};
use crate::resources::settings_menu::{SettingsItem, SettingsMenu, SettingsRequested, SettingsTab};
use crate::states::GameState;
use bevy::prelude::*;
use bevy::time::Real;
use bevy::ui::FocusPolicy;
use log::warn;

/// All settings input and UI lives in this set, so the pause overlay and the
/// title screen can order their own `Esc` handling before it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SettingsSet;

/// How long a `-` / `+` button must be held before it starts repeating.
const HOLD_DELAY: f32 = 0.35;
/// How often a held `-` / `+` button repeats its coarse step.
const HOLD_REPEAT: f32 = 0.06;

pub struct SettingsMenuPlugin;

impl Plugin for SettingsMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Settings>()
            .init_resource::<SettingsMenu>()
            .init_resource::<TextScaleState>()
            .init_resource::<MenuArt>()
            .init_resource::<AudioSettings>()
            .add_message::<SettingsRequested>()
            .add_message::<SettingsChanged>()
            .add_message::<PlaySfx>()
            // Settings load before the title screen exists, so the first frame of
            // UI already reflects the player's preferences.
            .add_systems(OnEnter(GameState::LoadingAssets), load_settings)
            .add_systems(
                Update,
                (
                    open_from_main_menu,
                    open_from_pause,
                    settings_rebind_listen,
                    settings_input,
                    settings_tab_click,
                    settings_step_click,
                    settings_step_hold,
                    settings_slot_click,
                    settings_reset_click,
                    settings_back_click,
                    clear_open_frame,
                    sync_settings_ui,
                    refresh_settings_ui,
                    apply_audio_settings,
                    apply_text_scale,
                    capture_base_font_sizes,
                    autosave_settings,
                )
                    .chain()
                    .in_set(SettingsSet)
                    .before(theme::MenuStyleSet),
            )
            // The palette pass overrides the colours the style set just wrote, so
            // it must run after it.
            .add_systems(Update, apply_colorblind_palette.after(theme::MenuStyleSet))
            // Leaving a menu drops the overlay; it never outlives the screen it
            // was opened from.
            .add_systems(OnExit(GameState::MainMenu), close_settings)
            .add_systems(OnExit(GameState::Playing), close_settings);
    }
}

// --- Components --------------------------------------------------------------

#[derive(Component, Reflect, Debug, Default)]
pub struct SettingsRoot;

/// The tab the overlay was built for, so a tab change can rebuild it.
#[derive(Component, Debug, Clone, Copy)]
struct SpawnedTab(SettingsTab);

/// A row container, highlighted when it is the current selection. Stepper rows
/// highlight on their own index; a control row highlights when either of its two
/// slots is selected.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub enum SettingsRowMarker {
    Row { index: usize },
    Control { primary: usize, secondary: usize },
}

/// A `-` / `+` button belonging to a row.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct SettingStep {
    pub row: SettingRow,
    pub delta: i32,
    pub index: usize,
}

/// Tracks how long a step button has been held, so a click steps finely and a
/// hold repeats coarsely.
#[derive(Component, Debug, Default)]
pub struct StepHold {
    elapsed: f32,
    since_repeat: f32,
}

/// How a button works out whether it is highlighted.
#[derive(Component, Debug, Clone, Copy)]
pub enum Selectable {
    /// Selected while its tab is the active one.
    Tab(SettingsTab),
    /// Selected while the highlight is on this flat index.
    Index(usize),
    /// Selected while the highlight is on the Back entry.
    Back,
}

/// A tab button in the tab bar.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct TabButton {
    pub tab: SettingsTab,
}

/// One of an action's two key buttons.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct SlotButton {
    pub action: Action,
    pub slot: KeySlot,
    pub index: usize,
}

/// Text showing a slot's bound key (or a prompt while it is being rebound).
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct SlotLabel {
    pub action: Action,
    pub slot: KeySlot,
}

/// The "reset keybindings" entry.
#[derive(Component, Reflect, Debug, Default)]
pub struct ResetButton;

/// Text that tracks a row's value, or the selected entry's hint.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsText {
    Value(SettingRow),
    Hint,
}

#[derive(Component, Reflect, Debug, Default)]
pub struct SettingsBackButton;

/// Emitted whenever the player changes a setting, so it can be saved.
#[derive(Message, Debug, Clone, Copy)]
pub struct SettingsChanged;

// --- Loading and fan-out -----------------------------------------------------

/// Loads the settings slot into the live resource and applies the parts that
/// live outside it (the audio bus and the text scale baseline).
fn load_settings(
    manager: Option<Res<SaveManager>>,
    mut settings: ResMut<Settings>,
    mut audio: ResMut<AudioSettings>,
    mut scale: ResMut<TextScaleState>,
) {
    let loaded = manager
        .map(|manager| manager.load_settings())
        .unwrap_or_default();
    *settings = loaded;
    *audio = settings.audio;
    scale.applied = settings.display.text_scale;
}

/// Copies the persisted audio mix onto the bus the audio layer reads.
fn apply_audio_settings(settings: Res<Settings>, mut audio: ResMut<AudioSettings>) {
    if settings.is_changed() && *audio != settings.audio {
        *audio = settings.audio;
    }
}

/// Persists the settings whenever the player changes one.
fn autosave_settings(
    manager: Option<Res<SaveManager>>,
    settings: Res<Settings>,
    mut changed: MessageReader<SettingsChanged>,
) {
    if changed.read().count() == 0 {
        return;
    }
    let Some(manager) = manager else {
        return;
    };
    if let Err(error) = manager.save_settings(&settings) {
        warn!("could not save settings: {error}");
    }
}

// --- Text scale --------------------------------------------------------------

/// The text scale currently baked into every font, so a change can be applied
/// as a fresh absolute size rather than compounding.
#[derive(Resource, Debug)]
struct TextScaleState {
    applied: f32,
}

impl Default for TextScaleState {
    // `#[derive(Default)]` would give 0.0, which would erase every font.
    fn default() -> Self {
        Self { applied: 1.0 }
    }
}

/// The authored size of a font, remembered so the scale can be re-applied.
#[derive(Component, Debug)]
struct BaseFontSize(FontSize);

/// Bakes the current scale into any font that has just appeared, recording its
/// authored size first.
fn capture_base_font_sizes(
    settings: Res<Settings>,
    mut commands: Commands,
    mut fonts: Query<(Entity, &mut TextFont), Added<TextFont>>,
) {
    let scale = settings.display.text_scale;
    for (entity, mut font) in fonts.iter_mut() {
        let base = font.font_size;
        commands.entity(entity).insert(BaseFontSize(base));
        font.font_size = base * scale;
    }
}

/// Re-applies the scale to every known font when the player changes it.
fn apply_text_scale(
    settings: Res<Settings>,
    mut state: ResMut<TextScaleState>,
    mut fonts: Query<(&mut TextFont, &BaseFontSize)>,
) {
    let scale = settings.display.text_scale;
    if (state.applied - scale).abs() < f32::EPSILON {
        return;
    }
    state.applied = scale;
    for (mut font, base) in fonts.iter_mut() {
        font.font_size = base.0 * scale;
    }
}

// --- Colourblind palette -----------------------------------------------------

/// Remaps the handful of semantic UI colours to the selected palette. Runs every
/// frame so newly spawned UI is covered too; the mapping is idempotent because
/// the target colours never equal the base ones.
fn apply_colorblind_palette(
    settings: Res<Settings>,
    mut texts: Query<&mut TextColor>,
    mut borders: Query<&mut BorderColor>,
) {
    let mode = settings.accessibility.colorblind;
    if mode == ColorblindMode::Off {
        return;
    }
    let palette = theme::palette(mode);

    for mut color in texts.iter_mut() {
        if color.0 == theme::TEXT_CRAFTABLE {
            color.0 = palette.craftable;
        } else if color.0 == theme::TEXT_BLOCKED {
            color.0 = palette.blocked;
        } else if color.0 == theme::TEXT_LOCKED {
            color.0 = palette.locked;
        }
    }
    for mut border in borders.iter_mut() {
        if border.top == theme::ROW_BORDER_SELECTED {
            border.set_all(palette.selected_border);
        } else if border.top == theme::ROW_BORDER_LOCKED {
            border.set_all(palette.locked_border);
        }
    }
}

// --- Opening -----------------------------------------------------------------

fn open_from_main_menu(
    mut requests: MessageReader<SettingsRequested>,
    mut menu: ResMut<SettingsMenu>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if requests.read().count() == 0 || menu.open {
        return;
    }
    menu.open_menu();
    sfx.write(PlaySfx(Sfx::MenuOpen));
}

/// Opens the overlay from the pause menu: the `Settings` button is clicked, or
/// it is highlighted and confirmed.
fn open_from_pause(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<SettingsMenu>,
    pause: Res<crate::resources::pause::PauseMenu>,
    confirm: Res<crate::resources::confirm::ConfirmDialog>,
    clicks: Query<(&Interaction, &PauseButton), Changed<Interaction>>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if menu.open || !pause.open || confirm.open {
        return;
    }
    let mut clicked = false;
    for (interaction, button) in clicks.iter() {
        if button.action == PauseAction::Settings && *interaction == Interaction::Pressed {
            clicked = true;
        }
    }
    let opened_by_key = confirm_keys(&keys) && pause.selected == BUTTON_SETTINGS;
    if clicked || opened_by_key {
        menu.open_menu();
        sfx.write(PlaySfx(Sfx::MenuOpen));
    }
}

// --- Input -------------------------------------------------------------------

/// Captures the next key for the slot being rebound, before the rest of the
/// settings input sees it.
fn settings_rebind_listen(
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut menu: ResMut<SettingsMenu>,
    mut settings: ResMut<Settings>,
    mut changed: MessageWriter<SettingsChanged>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let Some((action, slot)) = menu.listening else {
        return;
    };
    if !menu.open {
        menu.listening = None;
        return;
    }
    if theme::back_pressed(&keys) {
        menu.listening = None;
        keys.clear_just_pressed(KeyCode::Escape);
        keys.clear_just_pressed(KeyCode::Tab);
        sfx.write(PlaySfx(Sfx::MenuClose));
        return;
    }
    let captured = keys.get_just_pressed().next().copied();
    let Some(code) = captured else {
        return;
    };
    let name = format!("{code:?}");
    // `set` steals the key from wherever else it was, so one key fires one action.
    settings.input.set(action, slot, name);
    menu.listening = None;
    // The captured key must not also drive the menu this frame.
    keys.clear_just_pressed(code);
    changed.write(SettingsChanged);
    sfx.write(PlaySfx(Sfx::MenuConfirm));
}

fn settings_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<SettingsMenu>,
    mut settings: ResMut<Settings>,
    mut changed: MessageWriter<SettingsChanged>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if !menu.open || menu.just_opened || menu.listening.is_some() {
        return;
    }

    // `[` / `]` cycle pages; `Tab` and `Esc` both mean back.
    if keys.just_pressed(KeyCode::BracketRight) {
        menu.tab = menu.tab.next(1);
        menu.selected = 0;
        sfx.write(PlaySfx(Sfx::MenuNavigate));
        return;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        menu.tab = menu.tab.next(-1);
        menu.selected = 0;
        sfx.write(PlaySfx(Sfx::MenuNavigate));
        return;
    }

    if theme::back_pressed(&keys) {
        menu.close_menu();
        sfx.write(PlaySfx(Sfx::MenuClose));
        return;
    }

    let enabled: Vec<usize> = (0..menu.navigable_len()).collect();
    let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW);
    let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS);
    if up {
        menu.selected = theme::step_enabled(&enabled, menu.selected, -1);
        sfx.write(PlaySfx(Sfx::MenuNavigate));
    } else if down {
        menu.selected = theme::step_enabled(&enabled, menu.selected, 1);
        sfx.write(PlaySfx(Sfx::MenuNavigate));
    }

    let left = keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyA);
    let right = keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyD);
    let confirm = confirm_keys(&keys);

    // On Controls, left/right walk between an action's primary and secondary
    // buttons; there is nothing to adjust there.
    if menu.tab == SettingsTab::Controls && (left || right) {
        menu.selected = theme::step_enabled(&enabled, menu.selected, if left { -1 } else { 1 });
        sfx.write(PlaySfx(Sfx::MenuNavigate));
        return;
    }

    let delta = if left {
        -1
    } else if right || confirm {
        1
    } else {
        0
    };
    if delta == 0 {
        return;
    }

    match menu.item(menu.selected) {
        Some(SettingsItem::Row(row)) => {
            row.adjust(&mut settings, delta);
            changed.write(SettingsChanged);
        }
        Some(SettingsItem::Binding(action, slot)) if confirm => {
            menu.listening = Some((action, slot));
            sfx.write(PlaySfx(Sfx::MenuOpen));
        }
        Some(SettingsItem::Reset) if confirm => {
            settings.input.reset_defaults();
            changed.write(SettingsChanged);
            sfx.write(PlaySfx(Sfx::MenuConfirm));
        }
        Some(SettingsItem::Back) if confirm => {
            menu.close_menu();
            sfx.write(PlaySfx(Sfx::MenuClose));
        }
        _ => {}
    }
}

fn settings_tab_click(
    tabs: Query<(&Interaction, &TabButton), Changed<Interaction>>,
    mut menu: ResMut<SettingsMenu>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if !menu.open {
        return;
    }
    for (interaction, tab) in tabs.iter() {
        if *interaction == Interaction::Pressed && menu.tab != tab.tab {
            menu.tab = tab.tab;
            menu.selected = 0;
            menu.listening = None;
            sfx.write(PlaySfx(Sfx::MenuNavigate));
        }
    }
}

/// A single click of a `-` / `+` button applies its fine step.
fn settings_step_click(
    mut rows: Query<(&Interaction, &SettingStep), Changed<Interaction>>,
    mut menu: ResMut<SettingsMenu>,
    mut settings: ResMut<Settings>,
    mut changed: MessageWriter<SettingsChanged>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if !menu.open || menu.just_opened || menu.listening.is_some() {
        return;
    }
    for (interaction, step) in rows.iter_mut() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        menu.selected = step.index;
        step.row.adjust_tap(&mut settings, step.delta);
        changed.write(SettingsChanged);
        sfx.write(PlaySfx(Sfx::MenuNavigate));
    }
}

/// Holding a volume's `-` / `+` button past the delay repeats its coarse step.
fn settings_step_hold(
    time: Res<Time<Real>>,
    mut buttons: Query<(&Interaction, &SettingStep, &mut StepHold)>,
    menu: Res<SettingsMenu>,
    mut settings: ResMut<Settings>,
    mut changed: MessageWriter<SettingsChanged>,
) {
    let dt = time.delta_secs();
    for (interaction, step, mut hold) in buttons.iter_mut() {
        // Only volumes repeat; a held toggle would flicker.
        let holding = menu.open && *interaction == Interaction::Pressed && step.row.is_volume();
        if !holding {
            hold.elapsed = 0.0;
            hold.since_repeat = 0.0;
            continue;
        }
        hold.elapsed += dt;
        if hold.elapsed < HOLD_DELAY {
            continue;
        }
        hold.since_repeat += dt;
        while hold.since_repeat >= HOLD_REPEAT {
            hold.since_repeat -= HOLD_REPEAT;
            step.row.adjust(&mut settings, step.delta);
            changed.write(SettingsChanged);
        }
    }
}

/// Clicking a slot button starts rebinding that slot.
fn settings_slot_click(
    buttons: Query<(&Interaction, &SlotButton), Changed<Interaction>>,
    mut menu: ResMut<SettingsMenu>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if !menu.open {
        return;
    }
    for (interaction, button) in buttons.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        menu.selected = button.index;
        menu.listening = Some((button.action, button.slot));
        sfx.write(PlaySfx(Sfx::MenuOpen));
    }
}

/// Clicking the reset entry restores the shipped bindings.
fn settings_reset_click(
    buttons: Query<&Interaction, (With<ResetButton>, Changed<Interaction>)>,
    mut menu: ResMut<SettingsMenu>,
    mut settings: ResMut<Settings>,
    mut changed: MessageWriter<SettingsChanged>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if !menu.open {
        return;
    }
    for interaction in buttons.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        menu.selected = menu.reset_index();
        settings.input.reset_defaults();
        changed.write(SettingsChanged);
        sfx.write(PlaySfx(Sfx::MenuConfirm));
    }
}

fn settings_back_click(
    buttons: Query<&Interaction, (With<SettingsBackButton>, Changed<Interaction>)>,
    mut menu: ResMut<SettingsMenu>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if !menu.open {
        return;
    }
    for interaction in buttons.iter() {
        if *interaction == Interaction::Pressed {
            menu.close_menu();
            sfx.write(PlaySfx(Sfx::MenuClose));
        }
    }
}

/// Clears the opening-frame guard after the input systems have had their say, so
/// the key that opened the overlay cannot also act on it.
fn clear_open_frame(mut menu: ResMut<SettingsMenu>) {
    if menu.just_opened {
        menu.just_opened = false;
    }
}

/// The confirm keys shared by every menu.
fn confirm_keys(keys: &ButtonInput<KeyCode>) -> bool {
    keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::KeyE)
}

/// Drops the overlay when the screen under it goes away.
fn close_settings(mut menu: ResMut<SettingsMenu>) {
    menu.close_menu();
}

// --- Overlay -----------------------------------------------------------------

fn sync_settings_ui(
    mut commands: Commands,
    menu: Res<SettingsMenu>,
    mut art: ResMut<MenuArt>,
    server: Option<Res<AssetServer>>,
    roots: Query<(Entity, &SpawnedTab), With<SettingsRoot>>,
) {
    match (
        menu.open,
        roots.iter().next().map(|(entity, tab)| (entity, *tab)),
    ) {
        (true, None) => spawn_settings(&mut commands, &menu, &mut art, server.as_deref()),
        // A tab change rebuilds the panel so only the active tab's rows exist.
        (true, Some((entity, spawned))) if spawned.0 != menu.tab => {
            commands.entity(entity).despawn();
            spawn_settings(&mut commands, &menu, &mut art, server.as_deref());
        }
        (false, Some((entity, _))) => {
            commands.entity(entity).despawn();
        }
        _ => {}
    }
}

fn spawn_settings(
    commands: &mut Commands,
    menu: &SettingsMenu,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
) {
    let tab = menu.tab;
    let back_index = menu.back_index();
    let reset_index = menu.reset_index();
    commands
        .spawn((
            Name::new("Settings Root"),
            SettingsRoot,
            SpawnedTab(tab),
            theme::screen_root_node(),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.62)),
            GlobalZIndex(650),
            FocusPolicy::Block,
            Interaction::default(),
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Name::new("Settings Panel"),
                    Node {
                        width: Val::Px(760.0),
                        max_width: Val::Percent(94.0),
                        max_height: Val::Percent(90.0),
                        min_height: Val::Px(0.0),
                        overflow: Overflow::clip(),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(8.0),
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
                        theme::label("SETTINGS", 30.0, theme::TEXT_PRIMARY),
                    ));

                    spawn_tab_bar(panel, art, server, tab);

                    match tab {
                        SettingsTab::Controls => {
                            for (index, action) in Action::REBINDABLE.iter().enumerate() {
                                spawn_control_row(panel, art, server, *action, index);
                            }
                            spawn_reset_button(panel, art, server, reset_index);
                        }
                        _ => {
                            for (index, row) in menu.rows().iter().enumerate() {
                                spawn_setting_row(panel, art, server, *row, index);
                            }
                        }
                    }

                    panel.spawn((
                        Name::new("Row Hint"),
                        SettingsText::Hint,
                        theme::label("", 13.0, theme::TEXT_DIM),
                    ));

                    let spec = ButtonSpec::new(back_index, "Back").label_size(20.0);
                    let visuals = art.resolve(&spec, server);
                    theme::spawn_menu_button(
                        panel,
                        spec,
                        visuals,
                        (SettingsBackButton, Selectable::Back),
                        |_| {},
                    );

                    panel.spawn((
                        Name::new("Hint"),
                        theme::label(
                            "[ / ] pages   |   Arrows select   |   Left / Right adjust   |   Enter / click   |   Esc / Tab back",
                            13.0,
                            theme::TEXT_DIM,
                        ),
                    ));
                });
        });
}

/// The row of page buttons at the top of the panel.
fn spawn_tab_bar(
    panel: &mut ChildSpawnerCommands,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
    active: SettingsTab,
) {
    panel
        .spawn((
            Name::new("Tab Bar"),
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                ..default()
            },
        ))
        .with_children(|bar| {
            for (index, tab) in SettingsTab::ALL.iter().enumerate() {
                spawn_tab_button(bar, art, server, *tab, index, *tab == active);
            }
        });
}

fn spawn_tab_button(
    bar: &mut ChildSpawnerCommands,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
    tab: SettingsTab,
    index: usize,
    active: bool,
) {
    let visuals = art.shared_visuals(server);
    bar.spawn((
        Name::new(format!("Tab {}", tab.label())),
        MenuButton {
            index,
            enabled: true,
        },
        MenuButtonSelected(active),
        visuals,
        Button,
        Interaction::default(),
        FocusPolicy::Block,
        Node {
            flex_grow: 1.0,
            height: Val::Px(36.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(theme::ROW_BG),
        BorderColor::all(theme::ROW_BORDER),
        ImageNode::default(),
        TabButton { tab },
        Selectable::Tab(tab),
    ))
    .with_children(|button| {
        button.spawn(theme::label(tab.label(), 16.0, theme::TEXT_PRIMARY));
    });
}

fn spawn_setting_row(
    parent: &mut ChildSpawnerCommands,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
    row: SettingRow,
    index: usize,
) {
    parent
        .spawn((
            Name::new(format!("Setting {}", row.label())),
            SettingsRowMarker::Row { index },
            row_node(),
            BackgroundColor(theme::ROW_BG),
            BorderColor::all(theme::ROW_BORDER),
        ))
        .with_children(|node| {
            node.spawn((
                Node {
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    ..default()
                },
                theme::label(row.label(), 18.0, theme::TEXT_PRIMARY),
            ));
            spawn_step_button(node, art, server, "-", row, -1, index);
            node.spawn((
                SettingsText::Value(row),
                Node {
                    width: Val::Px(150.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                theme::label("", 16.0, theme::TEXT_PRIMARY),
            ));
            spawn_step_button(node, art, server, "+", row, 1, index);
        });
}

/// One action's row: its name, then a button for each of its two key slots.
fn spawn_control_row(
    parent: &mut ChildSpawnerCommands,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
    action: Action,
    action_index: usize,
) {
    let primary = SettingsMenu::slot_index(action, KeySlot::Primary).unwrap_or(action_index * 2);
    let secondary =
        SettingsMenu::slot_index(action, KeySlot::Secondary).unwrap_or(action_index * 2 + 1);
    parent
        .spawn((
            Name::new(format!("Control {}", action.label())),
            SettingsRowMarker::Control { primary, secondary },
            row_node(),
            BackgroundColor(theme::ROW_BG),
            BorderColor::all(theme::ROW_BORDER),
        ))
        .with_children(|node| {
            node.spawn((
                Node {
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    ..default()
                },
                theme::label(action.label(), 18.0, theme::TEXT_PRIMARY),
            ));
            spawn_slot_button(node, art, server, action, KeySlot::Primary, primary);
            spawn_slot_button(node, art, server, action, KeySlot::Secondary, secondary);
        });
}

/// The shared frame of a settings row.
fn row_node() -> Node {
    Node {
        width: Val::Percent(100.0),
        min_height: Val::Px(theme::ROW_HEIGHT),
        padding: UiRect::px(12.0, 12.0, 6.0, 6.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::SpaceBetween,
        column_gap: Val::Px(10.0),
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(4.0)),
        ..default()
    }
}

/// A small square stepper button, styled by the shared button systems.
fn spawn_step_button(
    parent: &mut ChildSpawnerCommands,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
    glyph: &str,
    row: SettingRow,
    delta: i32,
    index: usize,
) {
    let visuals = art.shared_visuals(server);
    parent
        .spawn((
            MenuButton {
                index: 0,
                enabled: true,
            },
            MenuButtonSelected(false),
            visuals,
            Button,
            Interaction::default(),
            FocusPolicy::Block,
            Node {
                width: Val::Px(40.0),
                height: Val::Px(32.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(theme::ROW_BG),
            BorderColor::all(theme::ROW_BORDER),
            ImageNode::default(),
            SettingStep { row, delta, index },
            StepHold::default(),
        ))
        .with_children(|button| {
            button.spawn(theme::label(glyph, 20.0, theme::TEXT_PRIMARY));
        });
}

/// A key slot button: its label shows the bound key.
fn spawn_slot_button(
    parent: &mut ChildSpawnerCommands,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
    action: Action,
    slot: KeySlot,
    index: usize,
) {
    let visuals = art.shared_visuals(server);
    parent
        .spawn((
            MenuButton {
                index,
                enabled: true,
            },
            MenuButtonSelected(false),
            visuals,
            Button,
            Interaction::default(),
            FocusPolicy::Block,
            Node {
                width: Val::Px(130.0),
                height: Val::Px(32.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(theme::ROW_BG),
            BorderColor::all(theme::ROW_BORDER),
            ImageNode::default(),
            SlotButton {
                action,
                slot,
                index,
            },
            Selectable::Index(index),
        ))
        .with_children(|button| {
            button.spawn((
                SlotLabel { action, slot },
                theme::label("", 15.0, theme::TEXT_PRIMARY),
            ));
        });
}

fn spawn_reset_button(
    parent: &mut ChildSpawnerCommands,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
    index: usize,
) {
    let spec = ButtonSpec::new(index, "Reset Keybindings").label_size(18.0);
    let visuals = art.resolve(&spec, server);
    theme::spawn_menu_button(
        parent,
        spec,
        visuals,
        (ResetButton, Selectable::Index(index)),
        |_| {},
    );
}

fn refresh_settings_ui(
    menu: Res<SettingsMenu>,
    settings: Res<Settings>,
    mut rows: Query<(&SettingsRowMarker, &mut BackgroundColor, &mut BorderColor)>,
    mut texts: Query<(&SettingsText, &mut Text), Without<SlotLabel>>,
    mut slots: Query<(&SlotLabel, &mut Text), Without<SettingsText>>,
    mut selectable: Query<(&Selectable, &mut MenuButtonSelected)>,
) {
    for (marker, mut background, mut border) in rows.iter_mut() {
        let selected = menu.open
            && match marker {
                SettingsRowMarker::Row { index } => *index == menu.selected,
                SettingsRowMarker::Control { primary, secondary } => {
                    menu.selected == *primary || menu.selected == *secondary
                }
            };
        background.0 = if selected {
            theme::ROW_BG_SELECTED
        } else {
            theme::ROW_BG
        };
        border.set_all(if selected {
            theme::ROW_BORDER_SELECTED
        } else {
            theme::ROW_BORDER
        });
    }

    for (marker, mut text) in texts.iter_mut() {
        match marker {
            SettingsText::Value(row) => *text = Text::new(row.value(&settings)),
            SettingsText::Hint => *text = Text::new(hint_for(&menu)),
        }
    }

    for (slot, mut text) in slots.iter_mut() {
        let shown = if menu.listening == Some((slot.action, slot.slot)) {
            "Press a key...".to_string()
        } else {
            settings.input.label(slot.action, slot.slot)
        };
        *text = Text::new(shown);
    }

    for (selectable, mut selected) in selectable.iter_mut() {
        selected.0 = menu.open
            && match selectable {
                Selectable::Tab(tab) => *tab == menu.tab,
                Selectable::Index(index) => *index == menu.selected,
                Selectable::Back => menu.selected == menu.back_index(),
            };
    }
}

fn hint_for(menu: &SettingsMenu) -> String {
    if let Some((action, slot)) = menu.listening {
        return format!(
            "Press a key for {} ({}), or Esc / Tab to cancel.",
            action.label(),
            slot.label()
        );
    }
    match menu.item(menu.selected) {
        Some(SettingsItem::Row(row)) => row.hint().to_string(),
        Some(SettingsItem::Binding(..)) => {
            "Select a slot's button, then press the key you want.".to_string()
        }
        Some(SettingsItem::Reset) => "Restore every key to its shipped binding.".to_string(),
        Some(SettingsItem::Back) => "Return to the previous menu.".to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::confirm::ConfirmDialog;
    use crate::resources::pause::PauseMenu;
    use bevy::state::app::StatesPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<PauseMenu>()
            .init_resource::<ConfirmDialog>()
            .add_plugins((MinimalPlugins, StatesPlugin, SettingsMenuPlugin))
            .init_state::<GameState>()
            .init_state::<crate::states::DayPhase>();
        // Settle the boot state so the one-shot `load_settings` has already run
        // and will not clobber a test's own settings mutations.
        app.update();
        app
    }

    fn roots(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<SettingsRoot>>()
            .iter(app.world())
            .count()
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

    fn open_menu(app: &mut App) {
        app.world_mut().resource_mut::<SettingsMenu>().open_menu();
        app.update();
        app.update();
    }

    fn open_controls(app: &mut App) {
        open_menu(app);
        app.world_mut().resource_mut::<SettingsMenu>().tab = SettingsTab::Controls;
        app.update();
        app.update();
    }

    #[test]
    fn opening_spawns_the_overlay_and_closing_drops_it() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<SettingsMenu>().open_menu();
        app.update();
        assert_eq!(roots(&mut app), 1);

        app.world_mut().resource_mut::<SettingsMenu>().close_menu();
        app.update();
        assert_eq!(roots(&mut app), 0);
    }

    #[test]
    fn the_main_menu_request_opens_the_overlay() {
        let mut app = setup_app();
        app.world_mut().write_message(SettingsRequested);
        app.update();
        assert!(app.world().resource::<SettingsMenu>().open);
        assert_eq!(roots(&mut app), 1);
    }

    #[test]
    fn the_pause_settings_button_opens_the_overlay() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<PauseMenu>().open_menu();
        app.world_mut().spawn((
            PauseButton {
                action: PauseAction::Settings,
                index: BUTTON_SETTINGS,
            },
            Interaction::Pressed,
        ));
        app.update();
        assert!(app.world().resource::<SettingsMenu>().open);
    }

    #[test]
    fn escape_closes_the_overlay() {
        let mut app = setup_app();
        open_menu(&mut app);
        tap(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<SettingsMenu>().open);
    }

    #[test]
    fn tab_closes_the_overlay_like_escape() {
        let mut app = setup_app();
        open_menu(&mut app);
        tap(&mut app, KeyCode::Tab);
        assert!(!app.world().resource::<SettingsMenu>().open);
    }

    #[test]
    fn the_opening_key_does_not_also_act() {
        let mut app = setup_app();
        app.world_mut().write_message(SettingsRequested);
        {
            let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            input.press(KeyCode::Enter);
        }
        app.update();
        assert_eq!(
            app.world().resource::<Settings>().audio.master,
            1.0,
            "the opening Enter does not step the first row"
        );
    }

    #[test]
    fn keyboard_adjusts_the_selected_row() {
        let mut app = setup_app();
        open_menu(&mut app);
        tap(&mut app, KeyCode::ArrowLeft);
        assert!((app.world().resource::<Settings>().audio.master - 0.9).abs() < 1e-6);
    }

    #[test]
    fn keyboard_walks_the_audio_rows_then_back() {
        let mut app = setup_app();
        open_menu(&mut app);
        for _ in 0..20 {
            tap(&mut app, KeyCode::ArrowDown);
        }
        let menu = app.world().resource::<SettingsMenu>();
        assert_eq!(menu.selected, menu.back_index(), "clamps at Back");
        assert_eq!(menu.back_index(), 3, "three audio rows then Back");
    }

    #[test]
    fn confirming_back_closes_the_overlay() {
        let mut app = setup_app();
        open_menu(&mut app);
        app.world_mut().resource_mut::<SettingsMenu>().selected = 3;
        app.update();
        tap(&mut app, KeyCode::Enter);
        assert!(!app.world().resource::<SettingsMenu>().open);
    }

    #[test]
    fn tab_cycles_the_page_and_resets_the_selection() {
        let mut app = setup_app();
        open_menu(&mut app);
        app.world_mut().resource_mut::<SettingsMenu>().selected = 2;
        app.update();

        tap(&mut app, KeyCode::BracketRight);
        assert_eq!(
            app.world().resource::<SettingsMenu>().tab,
            SettingsTab::Accessibility
        );
        assert_eq!(app.world().resource::<SettingsMenu>().selected, 0);
    }

    #[test]
    fn the_tab_stays_in_sync_with_the_rebuilt_panel() {
        let mut app = setup_app();
        open_menu(&mut app);
        for _ in 0..3 {
            tap(&mut app, KeyCode::BracketRight);
        }
        assert_eq!(
            app.world().resource::<SettingsMenu>().tab,
            SettingsTab::Controls
        );
        assert_eq!(roots(&mut app), 1, "the panel is rebuilt, not stacked");
    }

    #[test]
    fn changing_a_setting_emits_a_save_request() {
        let mut app = setup_app();
        open_menu(&mut app);
        tap(&mut app, KeyCode::ArrowRight);
        assert!(
            !app.world()
                .resource::<Messages<SettingsChanged>>()
                .is_empty(),
            "a change should queue a settings save"
        );
    }

    #[test]
    fn the_audio_bus_follows_the_settings() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<Settings>().audio.music = 0.3;
        app.update();
        assert_eq!(app.world().resource::<AudioSettings>().music, 0.3);
    }

    #[test]
    fn text_scale_rescales_existing_fonts() {
        let mut app = setup_app();
        let text = app
            .world_mut()
            .spawn((
                Text::new("hello"),
                TextFont {
                    font_size: FontSize::Px(20.0),
                    ..default()
                },
                TextColor(theme::TEXT_PRIMARY),
            ))
            .id();
        app.update();
        assert_eq!(
            app.world().get::<TextFont>(text).unwrap().font_size,
            FontSize::Px(20.0)
        );

        app.world_mut()
            .resource_mut::<Settings>()
            .display
            .text_scale = 1.5;
        app.update();
        assert_eq!(
            app.world().get::<TextFont>(text).unwrap().font_size,
            FontSize::Px(30.0)
        );

        app.world_mut()
            .resource_mut::<Settings>()
            .display
            .text_scale = 0.75;
        app.update();
        assert_eq!(
            app.world().get::<TextFont>(text).unwrap().font_size,
            FontSize::Px(15.0),
            "no compounding"
        );
    }

    #[test]
    fn the_colorblind_palette_remaps_semantic_colours() {
        let mut app = setup_app();
        let text = app.world_mut().spawn(TextColor(theme::TEXT_CRAFTABLE)).id();
        app.world_mut()
            .resource_mut::<Settings>()
            .accessibility
            .colorblind = ColorblindMode::Deuteranopia;
        app.update();
        assert_eq!(
            app.world().get::<TextColor>(text).unwrap().0,
            theme::palette(ColorblindMode::Deuteranopia).craftable
        );
    }

    /// A volume `-` button, pressed for one frame, applies the fine step.
    fn press_step(app: &mut App, row: SettingRow, delta: i32) {
        let visuals = app
            .world_mut()
            .resource_mut::<MenuArt>()
            .shared_visuals(None);
        let entity = app
            .world_mut()
            .spawn((
                MenuButton {
                    index: 0,
                    enabled: true,
                },
                MenuButtonSelected(false),
                visuals,
                Interaction::Pressed,
                BackgroundColor(theme::ROW_BG),
                BorderColor::all(theme::ROW_BORDER),
                ImageNode::default(),
                SettingStep {
                    row,
                    delta,
                    index: 0,
                },
                StepHold::default(),
            ))
            .id();
        app.update();
        app.world_mut().entity_mut(entity).despawn();
        app.update();
    }

    #[test]
    fn a_volume_click_steps_one_percent() {
        let mut app = setup_app();
        open_menu(&mut app);
        press_step(&mut app, SettingRow::MasterVolume, -1);
        assert!(
            (app.world().resource::<Settings>().audio.master - 0.99).abs() < 1e-6,
            "a single click moves one percent"
        );
    }

    #[test]
    fn a_held_volume_button_repeats_ten_percent() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;

        let mut app = setup_app();
        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            100,
        )));
        open_menu(&mut app);

        let visuals = app
            .world_mut()
            .resource_mut::<MenuArt>()
            .shared_visuals(None);
        app.world_mut().spawn((
            MenuButton {
                index: 0,
                enabled: true,
            },
            MenuButtonSelected(false),
            visuals,
            Interaction::Pressed,
            BackgroundColor(theme::ROW_BG),
            BorderColor::all(theme::ROW_BORDER),
            ImageNode::default(),
            SettingStep {
                row: SettingRow::MasterVolume,
                delta: -1,
                index: 0,
            },
            StepHold::default(),
        ));

        for _ in 0..12 {
            app.update();
        }
        let master = app.world().resource::<Settings>().audio.master;
        assert!(master < 0.95, "holding pulls the volume down, got {master}");
    }

    #[test]
    fn rebinding_an_actions_primary_slot() {
        let mut app = setup_app();
        open_controls(&mut app);
        app.world_mut().resource_mut::<SettingsMenu>().selected = 0; // MoveUp primary
        app.update();

        tap(&mut app, KeyCode::Enter);
        assert_eq!(
            app.world().resource::<SettingsMenu>().listening,
            Some((Action::MoveUp, KeySlot::Primary))
        );
        tap(&mut app, KeyCode::KeyJ);

        let settings = app.world().resource::<Settings>();
        assert_eq!(
            settings.input.binding(Action::MoveUp, KeySlot::Primary),
            Some("KeyJ")
        );
        assert!(
            app.world().resource::<SettingsMenu>().listening.is_none(),
            "listening ends once a key is captured"
        );
    }

    #[test]
    fn the_primary_and_secondary_slots_rebind_independently() {
        let mut app = setup_app();
        open_controls(&mut app);

        // Rebind the secondary (index 1) to KeyJ.
        app.world_mut().resource_mut::<SettingsMenu>().selected = 1;
        app.update();
        tap(&mut app, KeyCode::Enter);
        tap(&mut app, KeyCode::KeyJ);

        let settings = app.world().resource::<Settings>();
        assert_eq!(
            settings.input.binding(Action::MoveUp, KeySlot::Primary),
            Some("KeyW"),
            "the primary is untouched"
        );
        assert_eq!(
            settings.input.binding(Action::MoveUp, KeySlot::Secondary),
            Some("KeyJ")
        );
    }

    #[test]
    fn a_rebound_key_is_stolen_from_its_old_slot() {
        let mut app = setup_app();
        open_controls(&mut app);
        // Bind MoveDown's secondary (index 3) to KeyW, MoveUp's primary.
        app.world_mut().resource_mut::<SettingsMenu>().selected = 3;
        app.update();
        tap(&mut app, KeyCode::Enter);
        tap(&mut app, KeyCode::KeyW);

        let settings = app.world().resource::<Settings>();
        assert_eq!(
            settings.input.binding(Action::MoveUp, KeySlot::Primary),
            None,
            "KeyW left its old slot"
        );
        assert_eq!(
            settings.input.binding(Action::MoveDown, KeySlot::Secondary),
            Some("KeyW")
        );
    }

    #[test]
    fn escape_cancels_a_rebind_without_closing() {
        let mut app = setup_app();
        open_controls(&mut app);
        app.world_mut().resource_mut::<SettingsMenu>().listening =
            Some((Action::Interact, KeySlot::Primary));
        app.update();

        tap(&mut app, KeyCode::Escape);
        let menu = app.world().resource::<SettingsMenu>();
        assert!(menu.listening.is_none(), "the rebind is cancelled");
        assert!(menu.open, "the overlay stays open");
    }

    #[test]
    fn the_reset_entry_restores_the_shipped_bindings() {
        let mut app = setup_app();
        open_controls(&mut app);
        app.world_mut().resource_mut::<Settings>().input.set(
            Action::MoveUp,
            KeySlot::Primary,
            "KeyJ".to_string(),
        );
        app.update();

        app.world_mut().resource_mut::<SettingsMenu>().selected =
            app.world().resource::<SettingsMenu>().reset_index();
        app.update();
        tap(&mut app, KeyCode::Enter);

        let settings = app.world().resource::<Settings>();
        assert_eq!(
            settings.input.binding(Action::MoveUp, KeySlot::Primary),
            Some("KeyW"),
            "reset restores the shipped binding"
        );
    }
}
