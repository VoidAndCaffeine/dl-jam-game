//! The settings overlay.
//!
//! One overlay serves both the title screen and the pause menu: it reads the
//! shared [`Settings`] resource and writes it back, so there is no per-menu
//! variant. Rows are steppers (a `-` / value / `+` cluster); the keyboard walks
//! them with Up/Down and adjusts the highlighted one with Left/Right, while the
//! mouse drives the `-` / `+` buttons directly.
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
use crate::resources::settings::{ColorblindMode, SETTING_ROWS, SettingRow, Settings};
use crate::resources::settings_menu::{SettingsMenu, SettingsRequested};
use crate::states::GameState;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use log::warn;

/// All settings input and UI lives in this set, so the pause overlay and the
/// title screen can order their own `Esc` handling before it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SettingsSet;

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
                    settings_input,
                    settings_step_click,
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

/// A steppable settings row container.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct SettingRowMarker {
    pub row: SettingRow,
    pub index: usize,
}

/// A `-` / `+` button belonging to a row.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct SettingStep {
    pub row: SettingRow,
    pub delta: i32,
}

/// Text that tracks a row's value, or the selected row's hint.
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

fn settings_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<SettingsMenu>,
    mut settings: ResMut<Settings>,
    mut changed: MessageWriter<SettingsChanged>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if !menu.open || menu.just_opened {
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        menu.close_menu();
        sfx.write(PlaySfx(Sfx::MenuClose));
        return;
    }

    // Every row plus Back is selectable, so navigation is a plain clamp.
    let enabled: Vec<usize> = (0..=SettingsMenu::BACK).collect();
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
    if left || right {
        adjust_selected(
            &mut menu,
            &mut settings,
            &mut changed,
            if left { -1 } else { 1 },
        );
    }

    if confirm_keys(&keys) {
        if menu.selected == SettingsMenu::BACK {
            menu.close_menu();
            sfx.write(PlaySfx(Sfx::MenuClose));
        } else {
            adjust_selected(&mut menu, &mut settings, &mut changed, 1);
        }
    }
}

/// Steps the highlighted row (a no-op on the Back row).
fn adjust_selected(
    menu: &mut SettingsMenu,
    settings: &mut Settings,
    changed: &mut MessageWriter<SettingsChanged>,
    delta: i32,
) {
    let Some(row) = SETTING_ROWS.get(menu.selected).copied() else {
        return;
    };
    row.adjust(settings, delta);
    changed.write(SettingsChanged);
}

fn settings_step_click(
    mut rows: Query<(&Interaction, &SettingStep), Changed<Interaction>>,
    mut menu: ResMut<SettingsMenu>,
    mut settings: ResMut<Settings>,
    mut changed: MessageWriter<SettingsChanged>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if !menu.open || menu.just_opened {
        return;
    }
    for (interaction, step) in rows.iter_mut() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(index) = SETTING_ROWS.iter().position(|row| *row == step.row) {
            menu.selected = index;
        }
        step.row.adjust(&mut settings, step.delta);
        changed.write(SettingsChanged);
        sfx.write(PlaySfx(Sfx::MenuNavigate));
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
    settings: Res<Settings>,
    mut art: ResMut<MenuArt>,
    server: Option<Res<AssetServer>>,
    roots: Query<Entity, With<SettingsRoot>>,
) {
    let present = !roots.is_empty();
    if menu.open && !present {
        spawn_settings(&mut commands, &settings, &mut art, server.as_deref());
    } else if !menu.open && present {
        for entity in roots.iter() {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_settings(
    commands: &mut Commands,
    _settings: &Settings,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
) {
    commands
        .spawn((
            Name::new("Settings Root"),
            SettingsRoot,
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

                    spawn_section(panel, "AUDIO", art, server, &SETTING_ROWS[0..3]);
                    spawn_section(panel, "ACCESSIBILITY", art, server, &SETTING_ROWS[3..5]);
                    spawn_section(panel, "DISPLAY", art, server, &SETTING_ROWS[5..6]);

                    panel.spawn((
                        Name::new("Row Hint"),
                        SettingsText::Hint,
                        theme::label("", 13.0, theme::TEXT_DIM),
                    ));

                    let spec = ButtonSpec::new(SettingsMenu::BACK, "Back").label_size(20.0);
                    let visuals = art.resolve(&spec, server);
                    theme::spawn_menu_button(panel, spec, visuals, SettingsBackButton, |_| {});

                    panel.spawn((
                        Name::new("Hint"),
                        theme::label(
                            "Arrows select   |   Left / Right adjust   |   Enter / click steps   |   Esc back",
                            13.0,
                            theme::TEXT_DIM,
                        ),
                    ));
                });
        });
}

/// Spawns one titled group of setting rows.
fn spawn_section(
    panel: &mut ChildSpawnerCommands,
    title: &str,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
    rows: &[SettingRow],
) {
    panel.spawn((
        Name::new(format!("{title} Section")),
        theme::label(title, 15.0, theme::TEXT_DIM),
    ));
    for row in rows {
        let index = SETTING_ROWS.iter().position(|r| r == row).unwrap_or(0);
        spawn_setting_row(panel, art, server, *row, index);
    }
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
            SettingRowMarker { row, index },
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
            },
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
            spawn_step_button(node, art, server, "-", row, -1);
            node.spawn((
                SettingsText::Value(row),
                Node {
                    width: Val::Px(150.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                theme::label("", 16.0, theme::TEXT_PRIMARY),
            ));
            spawn_step_button(node, art, server, "+", row, 1);
        });
}

/// A small square stepper button, styled by the shared button systems.
fn spawn_step_button(
    parent: &mut ChildSpawnerCommands,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
    glyph: &str,
    row: SettingRow,
    delta: i32,
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
            SettingStep { row, delta },
        ))
        .with_children(|button| {
            button.spawn(theme::label(glyph, 20.0, theme::TEXT_PRIMARY));
        });
}

fn refresh_settings_ui(
    menu: Res<SettingsMenu>,
    settings: Res<Settings>,
    mut rows: Query<(&SettingRowMarker, &mut BackgroundColor, &mut BorderColor)>,
    mut texts: Query<(&SettingsText, &mut Text)>,
    mut back: Query<(&SettingsBackButton, &mut MenuButtonSelected)>,
) {
    for (marker, mut background, mut border) in rows.iter_mut() {
        let selected = menu.open && marker.index == menu.selected;
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
            SettingsText::Hint => *text = Text::new(hint_for(menu.selected)),
        }
    }

    for (_, mut selected) in back.iter_mut() {
        selected.0 = menu.open && menu.selected == SettingsMenu::BACK;
    }
}

fn hint_for(selected: usize) -> String {
    if selected == SettingsMenu::BACK {
        "Return to the previous menu.".to_string()
    } else {
        SETTING_ROWS
            .get(selected)
            .map(|row| row.hint().to_string())
            .unwrap_or_default()
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
    fn escape_closes_the_overlay() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<SettingsMenu>().open_menu();
        app.update();
        // The opening frame is guarded; let it pass.
        app.update();

        tap(&mut app, KeyCode::Escape);
        assert!(!app.world().resource::<SettingsMenu>().open);
    }

    #[test]
    fn the_opening_key_does_not_also_act() {
        let mut app = setup_app();
        // Open via the request, then press Enter on the same frame it opens.
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
        app.world_mut().resource_mut::<SettingsMenu>().open_menu();
        app.update();
        app.update();

        tap(&mut app, KeyCode::ArrowLeft);
        assert!((app.world().resource::<Settings>().audio.master - 0.9).abs() < 1e-6);
    }

    #[test]
    fn keyboard_walks_down_to_the_last_row_then_back() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<SettingsMenu>().open_menu();
        app.update();
        app.update();

        for _ in 0..20 {
            tap(&mut app, KeyCode::ArrowDown);
        }
        assert_eq!(
            app.world().resource::<SettingsMenu>().selected,
            SettingsMenu::BACK,
            "clamps at the Back button"
        );
    }

    #[test]
    fn confirming_back_closes_the_overlay() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<SettingsMenu>().open_menu();
        app.world_mut().resource_mut::<SettingsMenu>().selected = SettingsMenu::BACK;
        app.update();
        app.update();

        tap(&mut app, KeyCode::Enter);
        assert!(!app.world().resource::<SettingsMenu>().open);
    }

    #[test]
    fn changing_a_setting_emits_a_save_request() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<SettingsMenu>().open_menu();
        app.update();
        app.update();

        // Drain anything from opening, then change a value.
        tap(&mut app, KeyCode::ArrowRight);

        let messages = app.world().resource::<Messages<SettingsChanged>>();
        assert!(
            !messages.is_empty(),
            "a change should queue a settings save"
        );
    }

    #[test]
    fn the_audio_bus_follows_the_settings() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<Settings>().audio.music = 0.3;
        app.update();
        assert_eq!(
            app.world().resource::<AudioSettings>().music,
            0.3,
            "the audio bus mirrors the persisted mix"
        );
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
            FontSize::Px(30.0),
            "the font grows with the scale"
        );

        app.world_mut()
            .resource_mut::<Settings>()
            .display
            .text_scale = 0.75;
        app.update();
        assert_eq!(
            app.world().get::<TextFont>(text).unwrap().font_size,
            FontSize::Px(15.0),
            "and shrinks again without compounding"
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

        assert_ne!(
            app.world().get::<TextColor>(text).unwrap().0,
            theme::TEXT_CRAFTABLE,
            "the craftable green is remapped"
        );
        assert_eq!(
            app.world().get::<TextColor>(text).unwrap().0,
            theme::palette(ColorblindMode::Deuteranopia).craftable
        );
    }
}
