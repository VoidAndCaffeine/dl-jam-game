//! Shared widgets and styling for every menu.
//!
//! The whole point of this module is that menus never hard-code a look: a
//! button is a [`MenuButton`] carrying a [`MenuButtonVisuals`] resolved from
//! the shared 9-slice frame (or a per-button override), so dropping PNGs into
//! `assets/images/ui/buttons/` reskins the UI without touching layout code.
//! Until those files exist the colored fallback underneath is what you see.

use bevy::asset::AssetServer;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use std::collections::HashMap;

// --- Palette -----------------------------------------------------------------

pub const PANEL_BG: Color = Color::srgba(0.07, 0.07, 0.10, 0.96);
pub const PANEL_BORDER: Color = Color::srgba(0.55, 0.55, 0.68, 1.0);
pub const TEXT_PRIMARY: Color = Color::srgba(0.94, 0.94, 0.97, 1.0);
pub const TEXT_DIM: Color = Color::srgba(0.62, 0.63, 0.70, 1.0);
pub const TEXT_CRAFTABLE: Color = Color::srgba(0.44, 0.95, 0.52, 1.0);
pub const TEXT_BLOCKED: Color = Color::srgba(1.0, 0.76, 0.36, 1.0);
pub const TEXT_LOCKED: Color = Color::srgba(0.85, 0.45, 0.45, 1.0);

pub const ROW_BG: Color = Color::srgba(0.15, 0.15, 0.21, 0.95);
pub const ROW_BG_HOVER: Color = Color::srgba(0.26, 0.26, 0.36, 0.98);
pub const ROW_BG_SELECTED: Color = Color::srgba(0.30, 0.28, 0.14, 0.98);
pub const ROW_BG_DISABLED: Color = Color::srgba(0.10, 0.10, 0.13, 0.9);
pub const ROW_BORDER: Color = Color::srgba(0.30, 0.30, 0.38, 1.0);
pub const ROW_BORDER_SELECTED: Color = Color::srgb(1.0, 0.85, 0.20);
/// Border painted while a locked button flashes to refuse a click.
pub const ROW_BORDER_LOCKED: Color = Color::srgb(1.0, 0.25, 0.25);
/// How long the refusal flash stays lit.
pub const FLASH_SECONDS: f32 = 0.45;

/// The semantic UI colours, remapped as a group for colour-vision presets. The
/// base values are the constants above; a preset swaps only the ones a player
/// with that colour vision would struggle to tell apart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub craftable: Color,
    pub blocked: Color,
    pub locked: Color,
    pub selected_border: Color,
    pub locked_border: Color,
}

impl Palette {
    /// The untouched palette, equal to the constants above.
    pub const fn base() -> Self {
        Self {
            craftable: TEXT_CRAFTABLE,
            blocked: TEXT_BLOCKED,
            locked: TEXT_LOCKED,
            selected_border: ROW_BORDER_SELECTED,
            locked_border: ROW_BORDER_LOCKED,
        }
    }
}

/// The palette for a colour-vision preset. Red-green presets move the greens to
/// blue and the reds to violet; the blue-yellow preset moves the yellows to
/// pink and keeps the greens.
pub fn palette(mode: crate::resources::settings::ColorblindMode) -> Palette {
    use crate::resources::settings::ColorblindMode;
    match mode {
        ColorblindMode::Off => Palette::base(),
        ColorblindMode::Protanopia | ColorblindMode::Deuteranopia => Palette {
            craftable: Color::srgb(0.35, 0.65, 1.0),
            blocked: Color::srgb(1.0, 0.9, 0.3),
            locked: Color::srgb(0.85, 0.5, 1.0),
            selected_border: Color::srgb(0.4, 0.85, 1.0),
            locked_border: Color::srgb(0.85, 0.5, 1.0),
        },
        ColorblindMode::Tritanopia => Palette {
            craftable: Color::srgb(0.4, 0.9, 0.5),
            blocked: Color::srgb(1.0, 0.5, 0.7),
            locked: Color::srgb(1.0, 0.35, 0.35),
            selected_border: Color::srgb(0.95, 0.95, 0.95),
            locked_border: Color::srgb(1.0, 0.35, 0.35),
        },
    }
}

pub const PANEL_WIDTH: f32 = 1040.0;
pub const ROW_HEIGHT: f32 = 44.0;

/// The window height the UI is authored for. At this height every `Val::Px` and
/// `FontSize::Px` shows at its literal value; other heights scale the whole UI
/// (see [`responsive_ui_scale`]) so menus keep their proportions when the
/// window changes resolution. 1080p is what the panel and row sizes above were
/// designed for.
pub const UI_REFERENCE_HEIGHT: f32 = 1080.0;
/// Inset, in pixels, for the 9-slice frame's corners. Match this when authoring
/// the shared button PNGs so nothing stretches oddly.
pub const FRAME_SLICE: f32 = 14.0;

/// The three shared button frames every [`MenuButton`] uses unless it overrides
/// them. Drop these files in to reskin every button at once.
pub const FRAME_NORMAL: &str = "images/ui/buttons/frame_normal.png";
pub const FRAME_HOVERED: &str = "images/ui/buttons/frame_hovered.png";
pub const FRAME_SELECTED: &str = "images/ui/buttons/frame_selected.png";

// --- Button components -------------------------------------------------------

/// A clickable menu row. `index` is the row's position in its menu; disabled
/// rows (reserved slots such as Difficulty or Infinite Mode) are shown but not
/// navigable.
#[derive(Component, Reflect, Debug, Clone, Copy, Default)]
pub struct MenuButton {
    pub index: usize,
    pub enabled: bool,
}

/// Set by a menu's refresh system to mark the currently highlighted button.
#[derive(Component, Reflect, Debug, Clone, Copy, Default)]
pub struct MenuButtonSelected(pub bool);

/// A short red border flash a button shows when it refuses a press (a locked
/// `Difficulty` slot, for instance). Attach it to any [`MenuButton`] and call
/// [`MenuButtonFlash::trigger`]; the timer runs on real time so it still ticks
/// while virtual time is paused behind an overlay.
#[derive(Component, Reflect, Debug, Clone, Copy, Default)]
pub struct MenuButtonFlash {
    pub remaining: f32,
}

impl MenuButtonFlash {
    pub fn trigger(&mut self) {
        self.remaining = FLASH_SECONDS;
    }
}

/// The texture handles for a button's three visual states.
#[derive(Component, Debug, Clone, Default)]
pub struct MenuButtonVisuals {
    pub normal: Handle<Image>,
    pub hovered: Handle<Image>,
    pub selected: Handle<Image>,
}

/// Optional per-button override paths. `None` means "use the shared frame".
#[derive(Clone, Copy, Debug, Default)]
pub struct ButtonArt {
    pub normal: Option<&'static str>,
    pub hovered: Option<&'static str>,
    pub selected: Option<&'static str>,
}

/// A full description of a button to spawn.
pub struct ButtonSpec {
    pub index: usize,
    pub enabled: bool,
    /// Hidden buttons keep their layout slot but are invisible (reserved room).
    pub hidden: bool,
    pub art: ButtonArt,
    pub label: String,
    pub label_size: f32,
    /// Optional per-button art name. When set, the button loads
    /// `images/ui/buttons/<skin>_{normal,hovered,selected}.png` for any of
    /// those files that exist, falling back to the shared frame otherwise.
    pub skin: Option<&'static str>,
}

impl ButtonSpec {
    pub fn new(index: usize, label: impl Into<String>) -> Self {
        Self {
            index,
            enabled: true,
            hidden: false,
            art: ButtonArt::default(),
            label: label.into(),
            label_size: 20.0,
            skin: None,
        }
    }

    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self.enabled = false;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn label_size(mut self, size: f32) -> Self {
        self.label_size = size;
        self
    }

    pub fn skin(mut self, skin: &'static str) -> Self {
        self.skin = Some(skin);
        self
    }
}

include!(concat!(env!("OUT_DIR"), "/button_art.rs"));

/// Whether an override file is present (baked in by `build.rs`).
fn override_exists(file: &str) -> bool {
    BUTTON_ART_FILES.contains(&file)
}

/// Lazily loaded menu textures: the shared button frames plus any per-button
/// overrides, kept resident for the whole session (the set is small).
#[derive(Resource, Default)]
pub struct MenuArt {
    shared: Option<MenuButtonVisuals>,
    sheets: HashMap<String, Handle<Image>>,
}

impl MenuArt {
    /// Loads (and caches) a single UI texture for the session.
    pub fn texture(&mut self, path: &str, server: Option<&AssetServer>) -> Handle<Image> {
        self.load(path, server)
    }

    fn load(&mut self, path: &str, server: Option<&AssetServer>) -> Handle<Image> {
        match self.sheets.get(path) {
            Some(handle) => handle.clone(),
            None => {
                let handle = server
                    .map(|server| server.load(path.to_owned()))
                    .unwrap_or_default();
                self.sheets.insert(path.to_string(), handle.clone());
                handle
            }
        }
    }

    /// The shared frame handles, loading them once.
    pub fn shared(&mut self, server: Option<&AssetServer>) -> MenuButtonVisuals {
        if self.shared.is_none() {
            self.shared = Some(MenuButtonVisuals {
                normal: self.load(FRAME_NORMAL, server),
                hovered: self.load(FRAME_HOVERED, server),
                selected: self.load(FRAME_SELECTED, server),
            });
        }
        self.shared.clone().unwrap_or_default()
    }

    /// Resolves a button's visual handles: an explicit override path, then a
    /// per-button `skin` file if it exists, then the shared frame.
    pub fn resolve(
        &mut self,
        spec: &ButtonSpec,
        server: Option<&AssetServer>,
    ) -> MenuButtonVisuals {
        let shared = self.shared(server);
        MenuButtonVisuals {
            normal: self.state(spec, "normal", &shared.normal, server),
            hovered: self.state(spec, "hovered", &shared.hovered, server),
            selected: self.state(spec, "selected", &shared.selected, server),
        }
    }

    /// The shared frames for a button with no skin, such as a list row.
    pub fn shared_visuals(&mut self, server: Option<&AssetServer>) -> MenuButtonVisuals {
        self.shared(server)
    }

    fn state(
        &mut self,
        spec: &ButtonSpec,
        state: &str,
        shared: &Handle<Image>,
        server: Option<&AssetServer>,
    ) -> Handle<Image> {
        if let Some(path) = explicit_override(&spec.art, state) {
            return self.load(path, server);
        }
        if let Some(skin) = spec.skin {
            let file = format!("{skin}_{state}.png");
            if override_exists(&file) {
                return self.load(&format!("images/ui/buttons/{file}"), server);
            }
        }
        shared.clone()
    }
}

/// The explicit override path for `state`, if the button sets one.
fn explicit_override<'a>(art: &'a ButtonArt, state: &str) -> Option<&'a str> {
    match state {
        "normal" => art.normal,
        "hovered" => art.hovered,
        "selected" => art.selected,
        _ => None,
    }
}

/// Buttons are styled in this set, after every menu's refresh so a selection
/// change lands on the same frame it is made.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct MenuStyleSet;

/// The framework components of a menu button, without its label. Menus that
/// need a bespoke label (a name plus a status line) spawn this directly and add
/// their own children.
pub fn button_core(
    index: usize,
    enabled: bool,
    hidden: bool,
    visuals: MenuButtonVisuals,
) -> impl Bundle {
    let image = visuals.normal.clone();
    (
        MenuButton { index, enabled },
        MenuButtonSelected::default(),
        visuals,
        Button,
        Interaction::default(),
        FocusPolicy::Block,
        UiTransform::IDENTITY,
        Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(ROW_HEIGHT),
            padding: UiRect::px(14.0, 14.0, 6.0, 6.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            column_gap: Val::Px(10.0),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(if enabled { ROW_BG } else { ROW_BG_DISABLED }),
        BorderColor::all(ROW_BORDER),
        ImageNode {
            image,
            image_mode: sliced_frame(),
            ..default()
        },
        if hidden {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        },
    )
}

/// Spawns one menu button under `parent`, attaching `extra` (a menu's own
/// marker/action component) to the button entity and letting `children` add
/// extra rows (such as a have/need status line). Layout, background and border
/// are the fallback look; the [`MenuButtonVisuals`] frame draws on top once
/// loaded. Returns the button entity.
pub fn spawn_menu_button<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    spec: ButtonSpec,
    visuals: MenuButtonVisuals,
    extra: B,
    children: impl FnOnce(&mut ChildSpawnerCommands),
) -> Entity {
    let mut entity = parent.spawn((
        button_core(spec.index, spec.enabled, spec.hidden, visuals),
        extra,
    ));
    entity.with_children(|button| {
        button.spawn((
            Text::new(spec.label),
            TextFont {
                font_size: FontSize::Px(spec.label_size),
                ..default()
            },
            TextColor(if spec.enabled { TEXT_PRIMARY } else { TEXT_DIM }),
            MenuButtonLabel,
        ));
        children(button);
    });
    entity.id()
}

/// Marker on a button's label so menus can recolour it.
#[derive(Component, Reflect, Debug, Clone, Copy, Default)]
pub struct MenuButtonLabel;

fn sliced_frame() -> NodeImageMode {
    NodeImageMode::Sliced(TextureSlicer {
        border: BorderRect::all(FRAME_SLICE),
        center_scale_mode: SliceScaleMode::Stretch,
        sides_scale_mode: SliceScaleMode::Stretch,
        max_corner_scale: 1.0,
    })
}

/// Paints every menu button from its interaction, selection and enabled state.
#[allow(clippy::type_complexity)]
fn style_menu_buttons(
    mut buttons: Query<(
        &Interaction,
        &MenuButton,
        &MenuButtonSelected,
        &MenuButtonVisuals,
        Option<&MenuButtonFlash>,
        &mut ImageNode,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
) {
    for (interaction, button, selected, visuals, flash, mut image, mut bg, mut border) in
        buttons.iter_mut()
    {
        let flashing = flash.is_some_and(|flash| flash.remaining > 0.0);
        if !button.enabled && !flashing {
            swap_image(&mut image, &visuals.normal);
            bg.0 = ROW_BG_DISABLED;
            border.set_all(ROW_BORDER);
            continue;
        }
        let hovered = matches!(*interaction, Interaction::Hovered | Interaction::Pressed);
        // Selection owns the border and frame; hover lightens the fill
        // underneath, so a selected row still reads as hovered. A refused
        // press overrides the border with the locked red.
        bg.0 = if hovered {
            ROW_BG_HOVER
        } else if button.enabled {
            ROW_BG
        } else {
            ROW_BG_DISABLED
        };
        border.set_all(if flashing {
            ROW_BORDER_LOCKED
        } else if selected.0 {
            ROW_BORDER_SELECTED
        } else {
            ROW_BORDER
        });
        if selected.0 && button.enabled {
            swap_image(&mut image, &visuals.selected);
        } else if hovered {
            swap_image(&mut image, &visuals.hovered);
        } else {
            swap_image(&mut image, &visuals.normal);
        }
    }
}

/// Runs down every refusal flash. Uses real time so it keeps ticking while an
/// overlay has virtual time paused.
fn tick_menu_button_flash(time: Res<Time<Real>>, mut flashes: Query<&mut MenuButtonFlash>) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for mut flash in flashes.iter_mut() {
        flash.remaining = (flash.remaining - dt).max(0.0);
    }
}

/// How far a held button squashes down, and how fast it eases between sizes.
const PRESS_SCALE: f32 = 0.96;
const PRESS_EASE_RATE: f32 = 18.0;

/// Squashes a button slightly while the mouse holds it, so a press reads. Runs
/// on real time so a press under a paused overlay still animates.
fn animate_button_press(
    time: Res<Time<Real>>,
    mut buttons: Query<(&Interaction, &MenuButton, &mut UiTransform)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let blend = (1.0 - (-PRESS_EASE_RATE * dt).exp()).clamp(0.0, 1.0);
    for (interaction, button, mut transform) in buttons.iter_mut() {
        let target = if button.enabled && matches!(*interaction, Interaction::Pressed) {
            PRESS_SCALE
        } else {
            1.0
        };
        let current = transform.scale.x;
        if (current - target).abs() < 0.001 {
            transform.scale = Vec2::splat(target);
        } else {
            transform.scale = Vec2::splat(current + (target - current) * blend);
        }
    }
}

fn swap_image(image: &mut ImageNode, handle: &Handle<Image>) {
    if image.image != *handle {
        image.image = handle.clone();
    }
}

// --- Animated previews -------------------------------------------------------

/// One or more sprite clips cycled inside a menu pane.
///
/// A single clip loops its frames (an idle pose). Two or more clips alternate
/// forever (the gear menu's light/heavy swing showcase). When `layout` is
/// `None` the preview is a single static image, which is what the not-yet
/// imported plant animations use.
#[derive(Component, Debug)]
pub struct AnimatedPreview {
    pub clips: Vec<Handle<Image>>,
    /// Index into `clips` of the clip currently playing.
    pub playing_clip: usize,
    pub frame: usize,
    pub timer: Timer,
    pub layout: Option<Handle<TextureAtlasLayout>>,
    pub frames_per_clip: usize,
}

impl AnimatedPreview {
    pub fn new(clips: Vec<Handle<Image>>, layout: Option<Handle<TextureAtlasLayout>>) -> Self {
        Self {
            clips,
            playing_clip: 0,
            frame: 0,
            timer: Timer::from_seconds(
                crate::components::player_sprite::FRAME_SECONDS,
                TimerMode::Repeating,
            ),
            layout,
            frames_per_clip: crate::components::player_sprite::FRAME_COUNT,
        }
    }

    /// A clip-less static image (no atlas), used for placeholder art.
    pub fn still(image: Handle<Image>) -> Self {
        Self {
            clips: vec![image],
            playing_clip: 0,
            frame: 0,
            timer: Timer::from_seconds(1.0, TimerMode::Repeating),
            layout: None,
            frames_per_clip: 1,
        }
    }

    /// Replaces the clip set, resetting playback only when it actually changed.
    pub fn set_clips(&mut self, clips: Vec<Handle<Image>>) {
        if self.clips == clips {
            return;
        }
        self.clips = clips;
        self.playing_clip = 0;
        self.frame = 0;
        self.timer.reset();
    }

    pub fn set_layout(&mut self, layout: Option<Handle<TextureAtlasLayout>>) {
        self.layout = layout;
    }
}

fn animate_previews(
    settings: Option<Res<crate::resources::settings::Settings>>,
    time: Res<Time>,
    mut previews: Query<(&mut AnimatedPreview, &mut ImageNode)>,
) {
    let dt = time.delta_secs();
    let reduced = settings
        .as_ref()
        .is_some_and(|settings| settings.accessibility.reduced_motion);
    for (mut preview, mut image) in previews.iter_mut() {
        if preview.clips.is_empty() {
            continue;
        }
        let current = preview.clips[preview.playing_clip].clone();
        if image.image != current {
            image.image = current;
        }
        image.texture_atlas = preview.layout.clone().map(|layout| TextureAtlas {
            layout,
            index: preview.frame.min(preview.frames_per_clip.saturating_sub(1)),
        });
        if !reduced {
            step_preview(&mut preview, dt);
        }
    }
}

/// Advances a preview's frame clock by `dt`. Pure so it can be tested without a
/// running app or a real clock.
pub fn step_preview(preview: &mut AnimatedPreview, dt: f32) {
    if preview.layout.is_none() || preview.frames_per_clip <= 1 || preview.clips.is_empty() {
        return;
    }
    preview.timer.tick(core::time::Duration::from_secs_f32(dt));
    if !preview.timer.just_finished() {
        return;
    }
    preview.frame += 1;
    if preview.frame >= preview.frames_per_clip {
        preview.frame = 0;
        // Alternate to the next clip; a single clip wraps onto itself.
        preview.playing_clip = (preview.playing_clip + 1) % preview.clips.len();
    }
}

// --- Layout helpers ----------------------------------------------------------

/// The full-screen flex container every menu root uses.
pub fn screen_root_node() -> Node {
    Node {
        position_type: PositionType::Absolute,
        width: Val::Percent(100.0),
        height: Val::Percent(100.0),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    }
}

/// The framed panel that sits in the middle of a menu screen.
///
/// The panel is a fixed width at the reference resolution but never wider than
/// the viewport, and it clips its children: however tall a menu's content grows
/// it stays inside the panel's frame (the panes inside scroll instead). Without
/// the clip a short window let the inner panes spill past the panel background.
pub fn panel_node() -> Node {
    Node {
        width: Val::Px(PANEL_WIDTH),
        max_width: Val::Percent(94.0),
        max_height: Val::Percent(88.0),
        min_height: Val::Px(0.0),
        overflow: Overflow::clip(),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(10.0),
        padding: UiRect::all(Val::Px(20.0)),
        border: UiRect::all(Val::Px(2.0)),
        border_radius: BorderRadius::all(Val::Px(8.0)),
        ..default()
    }
}

/// A horizontal row of panes.
///
/// `min_height: 0` lets this row (and so the panes in it) shrink below their
/// content, which is what turns a too-tall menu into scrolling panes rather
/// than a panel that overflows the screen.
pub fn columns_node() -> Node {
    Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        flex_shrink: 1.0,
        min_height: Val::Px(0.0),
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(18.0),
        overflow: Overflow::clip(),
        ..default()
    }
}

/// One pane filling `flex_basis` percent of the columns row.
pub fn pane_node(flex_basis: f32) -> Node {
    Node {
        flex_basis: Val::Percent(flex_basis),
        flex_grow: 0.0,
        flex_shrink: 1.0,
        min_height: Val::Px(0.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(8.0),
        padding: UiRect::all(Val::Px(12.0)),
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(6.0)),
        overflow: Overflow::clip(),
        ..default()
    }
}

pub fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

/// Wires up the shared widget systems. Called once from [`crate::plugins::UIPlugin`].
pub fn register(app: &mut App) {
    app.init_resource::<MenuArt>().add_systems(
        Update,
        (
            style_menu_buttons,
            animate_button_press,
            animate_previews,
            tick_menu_button_flash,
            responsive_ui_scale,
        )
            .in_set(MenuStyleSet),
    );
}

/// Moves a highlight one step through `enabled` (a sorted list of the menu's
/// selectable indices), clamping at both ends instead of wrapping. `current`
/// that is not itself enabled snaps back to the first entry.
pub fn step_enabled(enabled: &[usize], current: usize, direction: i32) -> usize {
    let Some(position) = enabled.iter().position(|index| *index == current) else {
        return enabled.first().copied().unwrap_or(current);
    };
    let target = position as i32 + direction;
    if target < 0 || target >= enabled.len() as i32 {
        return current;
    }
    enabled[target as usize]
}

/// Keeps the whole UI in proportion with the window height.
///
/// `UiScale` multiplies fixed (`Val::Px` / `FontSize::Px`) values only, so this
/// grows or shrinks every panel, button, row and font together while leaving
/// viewport units alone. That is what keeps the title art pinned to the screen
/// (it uses `Val::Vh`) while the buttons over it scale with the resolution.
/// Running every frame means resizing the window mid-game simply re-scales.
fn responsive_ui_scale(window: Query<&Window>, scale: Option<ResMut<UiScale>>) {
    let Some(mut scale) = scale else {
        // No UI plugin (headless tests): nothing to scale.
        return;
    };
    let Ok(window) = window.single() else {
        return;
    };
    let height = window.height();
    if height <= 0.0 {
        return;
    }
    // Clamped so an extreme window cannot make the UI unusably small or giant.
    let target = (height / UI_REFERENCE_HEIGHT).clamp(0.5, 3.0);
    if (scale.0 - target).abs() > f32::EPSILON {
        scale.0 = target;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use bevy::window::WindowResolution;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin));
        register(&mut app);
        app
    }

    #[test]
    fn the_shared_frames_are_stable_paths() {
        for path in [FRAME_NORMAL, FRAME_HOVERED, FRAME_SELECTED] {
            assert!(path.starts_with("images/ui/buttons/"));
        }
    }

    #[test]
    fn resolving_without_overrides_uses_the_shared_frames() {
        let mut art = MenuArt::default();
        let spec = ButtonSpec::new(0, "row");
        let resolved = art.resolve(&spec, None);
        let shared = art.shared(None);
        assert_eq!(resolved.normal, shared.normal);
        assert_eq!(resolved.selected, shared.selected);
    }

    #[test]
    fn a_button_without_a_skin_falls_back_to_the_shared_frames() {
        let mut art = MenuArt::default();
        let spec = ButtonSpec::new(0, "row").skin("definitely_missing_skin");
        let resolved = art.resolve(&spec, None);
        let shared = art.shared(None);
        assert_eq!(resolved.normal, shared.normal);
    }

    #[test]
    fn override_exists_only_matches_pngs_in_the_folder() {
        // The shared frames are always present, so they are in the manifest.
        assert!(override_exists("frame_normal.png"));
        assert!(!override_exists("does_not_exist.png"));
    }

    #[test]
    fn selected_buttons_use_the_selected_frame() {
        let mut app = setup_app();
        let visuals = app.world_mut().resource_mut::<MenuArt>().shared(None);
        let entity = app
            .world_mut()
            .spawn((
                MenuButton {
                    index: 0,
                    enabled: true,
                },
                MenuButtonSelected(true),
                visuals,
                Interaction::default(),
                BackgroundColor(ROW_BG),
                BorderColor::all(ROW_BORDER),
                ImageNode::default(),
            ))
            .id();

        app.update();

        let border = app.world().get::<BorderColor>(entity).unwrap();
        assert_eq!(border.top, ROW_BORDER_SELECTED);
        assert_eq!(
            app.world().get::<BackgroundColor>(entity).unwrap().0,
            ROW_BG
        );
    }

    #[test]
    fn disabled_buttons_never_show_as_selected() {
        let mut app = setup_app();
        let visuals = app.world_mut().resource_mut::<MenuArt>().shared(None);
        let entity = app
            .world_mut()
            .spawn((
                MenuButton {
                    index: 1,
                    enabled: false,
                },
                MenuButtonSelected(true),
                visuals,
                Interaction::default(),
                BackgroundColor(ROW_BG),
                BorderColor::all(ROW_BORDER),
                ImageNode::default(),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<BackgroundColor>(entity).unwrap().0,
            ROW_BG_DISABLED
        );
    }

    #[test]
    fn a_refused_press_flashes_the_locked_border_then_fades() {
        let mut app = setup_app();
        let visuals = app.world_mut().resource_mut::<MenuArt>().shared(None);
        let entity = app
            .world_mut()
            .spawn((
                MenuButton {
                    index: 0,
                    enabled: false,
                },
                MenuButtonSelected(false),
                visuals,
                Interaction::default(),
                BackgroundColor(ROW_BG),
                BorderColor::all(ROW_BORDER),
                ImageNode::default(),
                MenuButtonFlash {
                    remaining: FLASH_SECONDS,
                },
            ))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<BorderColor>(entity).unwrap().top,
            ROW_BORDER_LOCKED,
            "a live flash overrides the border"
        );

        // Drain the flash and the disabled button is plain again.
        app.world_mut()
            .get_mut::<MenuButtonFlash>(entity)
            .unwrap()
            .remaining = 0.0;
        app.update();
        assert_eq!(
            app.world().get::<BorderColor>(entity).unwrap().top,
            ROW_BORDER,
            "once it fades the disabled button is plain again"
        );
    }

    #[test]
    fn triggering_a_flash_lights_it_for_the_shared_duration() {
        let mut flash = MenuButtonFlash::default();
        flash.trigger();
        assert_eq!(flash.remaining, FLASH_SECONDS);
    }

    #[test]
    fn a_pressed_button_squashes_then_eases_back() {
        use bevy::time::TimeUpdateStrategy;
        use std::time::Duration;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                50,
            )));
        register(&mut app);
        // The first frame primes real time with a zero delta; run it before the
        // button exists so the next frame has a real step to ease with.
        app.update();

        let entity = app
            .world_mut()
            .spawn((
                MenuButton {
                    index: 0,
                    enabled: true,
                },
                Interaction::Pressed,
                UiTransform::IDENTITY,
            ))
            .id();

        app.update();
        let held = app.world().get::<UiTransform>(entity).unwrap().scale.x;
        assert!(held < 1.0, "a held button squashes down, got {held}");

        *app.world_mut().get_mut::<Interaction>(entity).unwrap() = Interaction::None;
        for _ in 0..20 {
            app.update();
        }
        let released = app.world().get::<UiTransform>(entity).unwrap().scale.x;
        assert!(
            (released - 1.0).abs() < 0.01,
            "releasing eases the button back, got {released}"
        );
    }

    #[test]
    fn step_enabled_walks_the_selectable_rows_without_wrapping() {
        assert_eq!(step_enabled(&[0, 1, 3], 0, 1), 1);
        assert_eq!(step_enabled(&[0, 1, 3], 1, 1), 3, "skips the disabled row");
        assert_eq!(step_enabled(&[0, 1, 3], 3, 1), 3, "clamps at the end");
        assert_eq!(step_enabled(&[0, 1, 3], 0, -1), 0, "clamps at the start");
        assert_eq!(
            step_enabled(&[0, 2], 1, 1),
            0,
            "a disabled current snaps to the first enabled row"
        );
    }

    #[test]
    fn a_single_clip_preview_loops_its_frames() {
        use crate::components::player_sprite::FRAME_COUNT;

        let mut preview = AnimatedPreview::new(vec![Handle::default()], Some(Handle::default()));
        preview.frames_per_clip = FRAME_COUNT;
        for _ in 0..FRAME_COUNT {
            step_preview(&mut preview, 0.2);
        }

        assert_eq!(preview.clips.len(), 1);
        assert_eq!(preview.frame, 0, "a single looping clip wraps");
        assert_eq!(preview.playing_clip, 0);
    }

    #[test]
    fn two_clips_alternate_forever() {
        let mut preview = AnimatedPreview::new(
            vec![Handle::default(), Handle::default()],
            Some(Handle::default()),
        );
        preview.frames_per_clip = 3;

        for _ in 0..3 {
            step_preview(&mut preview, 0.2);
        }
        assert_eq!(preview.playing_clip, 1, "advanced to the second clip");

        for _ in 0..3 {
            step_preview(&mut preview, 0.2);
        }
        assert_eq!(preview.playing_clip, 0, "wraps back to the first clip");
    }

    #[test]
    fn a_still_preview_does_not_animate() {
        let mut preview = AnimatedPreview::still(Handle::default());
        for _ in 0..10 {
            step_preview(&mut preview, 0.5);
        }
        assert_eq!(preview.frame, 0);
        assert_eq!(preview.playing_clip, 0);
    }

    #[test]
    fn panels_clip_their_content_and_stay_inside_the_viewport() {
        let node = panel_node();
        assert_eq!(node.overflow, Overflow::clip());
        assert_eq!(node.width, Val::Px(PANEL_WIDTH));
        assert_eq!(node.max_width, Val::Percent(94.0));
        assert_eq!(node.max_height, Val::Percent(88.0));
        assert_eq!(
            node.min_height,
            Val::Px(0.0),
            "the panel can shrink to its max height"
        );
        assert_eq!(node.height, Val::Auto, "height is content-driven");
    }

    #[test]
    fn the_columns_row_and_panes_can_shrink_so_tall_lists_scroll() {
        let columns = columns_node();
        assert_eq!(columns.min_height, Val::Px(0.0));
        assert_eq!(columns.flex_shrink, 1.0);
        assert_eq!(columns.flex_grow, 1.0);

        let pane = pane_node(40.0);
        assert_eq!(pane.min_height, Val::Px(0.0));
        assert_eq!(pane.flex_shrink, 1.0);
    }

    #[test]
    fn the_ui_scales_with_the_window_height() {
        let mut app = setup_app();
        app.insert_resource(UiScale::default());
        let window = app
            .world_mut()
            .spawn(Window {
                resolution: WindowResolution::new(1280, 720),
                ..default()
            })
            .id();

        app.update();
        assert!(
            (app.world().resource::<UiScale>().0 - 720.0 / UI_REFERENCE_HEIGHT).abs() < 1e-4,
            "720p shrinks the UI below its reference size"
        );

        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution = WindowResolution::new(1920, 1080);
        app.update();
        assert!(
            (app.world().resource::<UiScale>().0 - 1.0).abs() < 1e-4,
            "the reference height renders at 1:1"
        );

        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution = WindowResolution::new(2560, 1440);
        app.update();
        assert!(
            (app.world().resource::<UiScale>().0 - 1440.0 / UI_REFERENCE_HEIGHT).abs() < 1e-4,
            "1440p scales the UI up"
        );
    }

    #[test]
    fn the_ui_scale_is_clamped_against_extreme_windows() {
        let mut app = setup_app();
        app.insert_resource(UiScale::default());
        let window = app
            .world_mut()
            .spawn(Window {
                resolution: WindowResolution::new(320, 100),
                ..default()
            })
            .id();

        app.update();
        assert!(
            app.world().resource::<UiScale>().0 >= 0.5,
            "a tiny window never shrinks the UI past the floor"
        );

        app.world_mut()
            .get_mut::<Window>(window)
            .unwrap()
            .resolution = WindowResolution::new(7680, 4320);
        app.update();
        assert!(
            app.world().resource::<UiScale>().0 <= 3.0,
            "a huge window never grows the UI past the ceiling"
        );
    }

    #[test]
    fn without_a_window_the_ui_scale_is_left_untouched() {
        let mut app = setup_app();
        app.insert_resource(UiScale(1.25));
        app.update();
        assert_eq!(app.world().resource::<UiScale>().0, 1.25);
    }
}
