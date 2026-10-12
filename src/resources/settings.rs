//! Player settings: audio mix, accessibility and display preferences.
//!
//! Settings live in their own save slot ([`SETTINGS_KEY`]) rather than in the run
//! save, so they survive New Game, death and every run. The payload carries a
//! [`SETTINGS_VERSION`]; when a build changes the shape of [`Settings`], loading
//! an older file still imports what it can and defaults the rest (see
//! [`Settings::decode`]) instead of throwing the player's preferences away.

use crate::resources::audio_settings::AudioSettings;
use bevy::input::ButtonInput;
use bevy::prelude::*;
use bincode::Options;
use log::warn;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// The save slot settings live under. Separate from the run's `"run"` slot.
pub const SETTINGS_KEY: &str = "settings";

/// Bumped when the settings payload's shape changes. Older files are migrated.
pub const SETTINGS_VERSION: u32 = 2;

/// A rebindable action. The bindings are stored (and migrated) with the settings
/// so a future key-rebinding UI has somewhere to write; nothing reads them yet.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Interact,
    LightAttack,
    HeavyAttack,
    OpenInventory,
    OpenCrafting,
    Pause,
}

impl Action {
    /// Every action, in the order a controls list would show them.
    pub const ALL: [Action; 10] = [
        Action::MoveUp,
        Action::MoveDown,
        Action::MoveLeft,
        Action::MoveRight,
        Action::Interact,
        Action::LightAttack,
        Action::HeavyAttack,
        Action::OpenInventory,
        Action::OpenCrafting,
        Action::Pause,
    ];

    /// The actions shown in the controls list. Pause is excluded: its key is the
    /// universal menu `Esc`, which every overlay relies on, so it stays fixed.
    pub const REBINDABLE: [Action; 9] = [
        Action::MoveUp,
        Action::MoveDown,
        Action::MoveLeft,
        Action::MoveRight,
        Action::Interact,
        Action::LightAttack,
        Action::HeavyAttack,
        Action::OpenInventory,
        Action::OpenCrafting,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Action::MoveUp => "Move Up",
            Action::MoveDown => "Move Down",
            Action::MoveLeft => "Move Left",
            Action::MoveRight => "Move Right",
            Action::Interact => "Interact",
            Action::LightAttack => "Light Attack",
            Action::HeavyAttack => "Heavy Attack",
            Action::OpenInventory => "Inventory",
            Action::OpenCrafting => "Crafting",
            Action::Pause => "Pause",
        }
    }
}

/// Which of an action's two bindings a key goes to.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySlot {
    Primary,
    Secondary,
}

impl KeySlot {
    pub const ALL: [KeySlot; 2] = [KeySlot::Primary, KeySlot::Secondary];

    pub fn label(self) -> &'static str {
        match self {
            KeySlot::Primary => "Primary",
            KeySlot::Secondary => "Secondary",
        }
    }
}

/// The two keys bound to one action. Either may be unset.
///
/// Keys are stored as stable names (a `KeyCode`'s debug name), so the payload
/// never depends on Bevy's own serialization feature.
#[derive(Reflect, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionBinding {
    pub action: Action,
    pub primary: Option<String>,
    pub secondary: Option<String>,
}

impl ActionBinding {
    pub fn get(&self, slot: KeySlot) -> Option<&String> {
        match slot {
            KeySlot::Primary => self.primary.as_ref(),
            KeySlot::Secondary => self.secondary.as_ref(),
        }
    }

    fn set(&mut self, slot: KeySlot, value: Option<String>) {
        match slot {
            KeySlot::Primary => self.primary = value,
            KeySlot::Secondary => self.secondary = value,
        }
    }
}

/// A primary and a secondary key per action. The defaults match the shipped
/// scheme.
#[derive(Reflect, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputMap {
    bindings: Vec<ActionBinding>,
}

impl Default for InputMap {
    fn default() -> Self {
        Self {
            bindings: Action::ALL
                .iter()
                .map(|action| {
                    let (primary, secondary) = default_binding(*action);
                    ActionBinding {
                        action: *action,
                        primary: primary.map(str::to_string),
                        secondary: secondary.map(str::to_string),
                    }
                })
                .collect(),
        }
    }
}

impl InputMap {
    fn find(&self, action: Action) -> Option<&ActionBinding> {
        self.bindings
            .iter()
            .find(|binding| binding.action == action)
    }

    fn find_mut(&mut self, action: Action) -> Option<&mut ActionBinding> {
        self.bindings
            .iter_mut()
            .find(|binding| binding.action == action)
    }

    /// The key name bound to `action`'s slot, if any.
    pub fn binding(&self, action: Action, slot: KeySlot) -> Option<&str> {
        self.find(action)
            .and_then(|binding| binding.get(slot))
            .map(String::as_str)
    }

    /// The bound key rendered for the controls list (`"W"`, or `"Unbound"`).
    pub fn label(&self, action: Action, slot: KeySlot) -> String {
        self.binding(action, slot)
            .map(key_label)
            .unwrap_or_else(|| "Unbound".to_string())
    }

    /// Binds `name` to `action`'s slot, stealing it from wherever else it was so
    /// one key never fires two actions.
    pub fn set(&mut self, action: Action, slot: KeySlot, name: String) {
        self.unbind_name(&name);
        if let Some(binding) = self.find_mut(action) {
            binding.set(slot, Some(name));
        }
    }

    /// Clears a slot.
    pub fn clear(&mut self, action: Action, slot: KeySlot) {
        if let Some(binding) = self.find_mut(action) {
            binding.set(slot, None);
        }
    }

    /// Removes `name` from every slot.
    fn unbind_name(&mut self, name: &str) {
        for binding in &mut self.bindings {
            if binding.primary.as_deref() == Some(name) {
                binding.primary = None;
            }
            if binding.secondary.as_deref() == Some(name) {
                binding.secondary = None;
            }
        }
    }

    /// Restores every key to its shipped binding.
    pub fn reset_defaults(&mut self) {
        *self = InputMap::default();
    }

    /// Whether `key` is bound to `action` (either slot).
    pub fn is_bound(&self, action: Action, key: KeyCode) -> bool {
        let name = format!("{key:?}");
        KeySlot::ALL
            .iter()
            .any(|slot| self.binding(action, *slot) == Some(name.as_str()))
    }

    /// Whether either of `action`'s keys is currently held.
    pub fn pressed(&self, keys: &ButtonInput<KeyCode>, action: Action) -> bool {
        let Some(binding) = self.find(action) else {
            return false;
        };
        keys.get_pressed().any(|code| {
            let name = format!("{code:?}");
            binding.primary.as_deref() == Some(name.as_str())
                || binding.secondary.as_deref() == Some(name.as_str())
        })
    }

    /// Whether either of `action`'s keys was pressed this frame.
    pub fn just_pressed(&self, keys: &ButtonInput<KeyCode>, action: Action) -> bool {
        let Some(binding) = self.find(action) else {
            return false;
        };
        keys.get_just_pressed().any(|code| {
            let name = format!("{code:?}");
            binding.primary.as_deref() == Some(name.as_str())
                || binding.secondary.as_deref() == Some(name.as_str())
        })
    }

    /// Imports the v1 shape (a list of keys per action) into primary/secondary.
    /// The first key becomes the primary, the second the secondary; an action the
    /// old map omitted keeps its shipped binding.
    fn from_v1(bindings: Vec<(Action, Vec<String>)>) -> Self {
        let mut map = InputMap {
            bindings: Vec::new(),
        };
        for action in Action::ALL {
            let names = bindings
                .iter()
                .find(|(bound, _)| *bound == action)
                .map(|(_, names)| names.clone())
                .filter(|names| !names.is_empty());
            let (primary, secondary) = match names {
                Some(mut names) => {
                    let primary = names.remove(0);
                    (Some(primary), names.first().cloned())
                }
                None => {
                    let (primary, secondary) = default_binding(action);
                    (primary.map(str::to_string), secondary.map(str::to_string))
                }
            };
            map.bindings.push(ActionBinding {
                action,
                primary,
                secondary,
            });
        }
        map
    }
}

/// Whether a rebindable action is currently held. Falls back to the shipped
/// bindings when settings are absent (a system wired up alone in a test).
pub fn action_pressed(
    settings: &Option<Res<Settings>>,
    keys: &ButtonInput<KeyCode>,
    action: Action,
) -> bool {
    bindings_of(settings.as_deref().map(|settings| &settings.input)).pressed(keys, action)
}

/// Whether a rebindable action was pressed this frame. See [`action_pressed`].
pub fn action_just_pressed(
    settings: &Option<Res<Settings>>,
    keys: &ButtonInput<KeyCode>,
    action: Action,
) -> bool {
    bindings_of(settings.as_deref().map(|settings| &settings.input)).just_pressed(keys, action)
}

/// The live input map, or the shipped one when settings are unavailable.
fn bindings_of(map: Option<&InputMap>) -> &InputMap {
    match map {
        Some(map) => map,
        None => default_input_map(),
    }
}

/// A process-wide default input map for the no-settings fallback.
fn default_input_map() -> &'static InputMap {
    static FALLBACK: OnceLock<InputMap> = OnceLock::new();
    FALLBACK.get_or_init(InputMap::default)
}

/// A key name (`"KeyW"`) rendered for the controls list (`"W"`).
pub fn key_label(name: &str) -> String {
    match name {
        "ArrowUp" => "Up".to_string(),
        "ArrowDown" => "Down".to_string(),
        "ArrowLeft" => "Left".to_string(),
        "ArrowRight" => "Right".to_string(),
        "ShiftLeft" | "ShiftRight" => "Shift".to_string(),
        "ControlLeft" | "ControlRight" => "Ctrl".to_string(),
        "AltLeft" | "AltRight" => "Alt".to_string(),
        other => {
            for prefix in ["Key", "Digit", "Numpad"] {
                if let Some(rest) = other.strip_prefix(prefix) {
                    return rest.to_string();
                }
            }
            other.to_string()
        }
    }
}

/// The shipped bindings for one action, as `(primary, secondary)` key names.
fn default_binding(action: Action) -> (Option<&'static str>, Option<&'static str>) {
    match action {
        Action::MoveUp => (Some("KeyW"), Some("ArrowUp")),
        Action::MoveDown => (Some("KeyS"), Some("ArrowDown")),
        Action::MoveLeft => (Some("KeyA"), Some("ArrowLeft")),
        Action::MoveRight => (Some("KeyD"), Some("ArrowRight")),
        Action::Interact => (Some("Space"), None),
        Action::LightAttack => (Some("KeyQ"), None),
        Action::HeavyAttack => (Some("KeyE"), None),
        Action::OpenInventory => (Some("KeyI"), None),
        Action::OpenCrafting => (Some("KeyC"), None),
        Action::Pause => (Some("Escape"), None),
    }
}

/// A colour-vision preset. Each remaps the handful of semantic UI colours so
/// "craftable", "blocked" and "selected" stay distinguishable.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ColorblindMode {
    #[default]
    Off,
    Protanopia,
    Deuteranopia,
    Tritanopia,
}

impl ColorblindMode {
    pub const ALL: [ColorblindMode; 4] = [
        ColorblindMode::Off,
        ColorblindMode::Protanopia,
        ColorblindMode::Deuteranopia,
        ColorblindMode::Tritanopia,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ColorblindMode::Off => "Off",
            ColorblindMode::Protanopia => "Protanopia",
            ColorblindMode::Deuteranopia => "Deuteranopia",
            ColorblindMode::Tritanopia => "Tritanopia",
        }
    }

    /// Steps to the next preset, wrapping around.
    pub fn next(self, delta: i32) -> Self {
        let index = ColorblindMode::ALL
            .iter()
            .position(|mode| *mode == self)
            .unwrap_or(0) as i32;
        let count = ColorblindMode::ALL.len() as i32;
        let target = (index + delta).rem_euclid(count) as usize;
        ColorblindMode::ALL[target]
    }
}

/// Motion- and colour-related preferences.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AccessibilitySettings {
    /// Disables screen shake, hit pause and particles.
    pub reduced_motion: bool,
    pub colorblind: ColorblindMode,
}

impl Default for AccessibilitySettings {
    fn default() -> Self {
        Self {
            reduced_motion: false,
            colorblind: ColorblindMode::Off,
        }
    }
}

/// Display preferences.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DisplaySettings {
    /// Multiplier applied to every UI font size, 0.75..=1.5.
    pub text_scale: f32,
}

/// The allowed range for [`DisplaySettings::text_scale`].
pub const TEXT_SCALE_MIN: f32 = 0.75;
pub const TEXT_SCALE_MAX: f32 = 1.5;
pub const TEXT_SCALE_STEP: f32 = 0.25;

/// A volume's coarse step: a keyboard press, or one repeat of a held click.
pub const VOLUME_STEP: f32 = 0.1;
/// A volume's fine step: a single click of a `-` / `+` button.
pub const VOLUME_TAP_STEP: f32 = 0.01;

impl Default for DisplaySettings {
    fn default() -> Self {
        Self { text_scale: 1.0 }
    }
}

/// The whole settings payload, persisted under [`SETTINGS_KEY`].
#[derive(Resource, Reflect, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// The payload's schema version. Always the first field, so [`decode`] can
    /// read it straight off the front of the bytes.
    pub version: u32,
    pub audio: AudioSettings,
    pub accessibility: AccessibilitySettings,
    pub display: DisplaySettings,
    /// Key bindings, read by gameplay and written by the controls list.
    #[reflect(ignore)]
    pub input: InputMap,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            audio: AudioSettings::default(),
            accessibility: AccessibilitySettings::default(),
            display: DisplaySettings::default(),
            input: InputMap::default(),
        }
    }
}

/// A settings row the menu can step. `Back` is not a row: it is a button.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingRow {
    MasterVolume,
    MusicVolume,
    SfxVolume,
    ReducedMotion,
    Colorblind,
    TextScale,
}

/// The rows of the Audio tab.
pub const AUDIO_ROWS: &[SettingRow] = &[
    SettingRow::MasterVolume,
    SettingRow::MusicVolume,
    SettingRow::SfxVolume,
];
/// The rows of the Accessibility tab.
pub const ACCESSIBILITY_ROWS: &[SettingRow] = &[SettingRow::ReducedMotion, SettingRow::Colorblind];
/// The rows of the Display tab.
pub const DISPLAY_ROWS: &[SettingRow] = &[SettingRow::TextScale];

impl SettingRow {
    pub fn label(self) -> &'static str {
        match self {
            SettingRow::MasterVolume => "Master Volume",
            SettingRow::MusicVolume => "Music Volume",
            SettingRow::SfxVolume => "SFX Volume",
            SettingRow::ReducedMotion => "Reduced Motion",
            SettingRow::Colorblind => "Colourblind Mode",
            SettingRow::TextScale => "Text Size",
        }
    }

    /// A one-line description of what the row does.
    pub fn hint(self) -> &'static str {
        match self {
            SettingRow::MasterVolume => "Overall loudness of music and effects.",
            SettingRow::MusicVolume => "Volume of the background music.",
            SettingRow::SfxVolume => "Volume of sound effects.",
            SettingRow::ReducedMotion => "Turns off screen shake, hit pause and particles.",
            SettingRow::Colorblind => "Remaps the craftable / blocked / selected colours.",
            SettingRow::TextScale => "Scales every menu and HUD font.",
        }
    }

    /// The value shown on the right of the row.
    pub fn value(self, settings: &Settings) -> String {
        match self {
            SettingRow::MasterVolume => percent(settings.audio.master),
            SettingRow::MusicVolume => percent(settings.audio.music),
            SettingRow::SfxVolume => percent(settings.audio.sfx),
            SettingRow::ReducedMotion => {
                if settings.accessibility.reduced_motion {
                    "On".to_string()
                } else {
                    "Off".to_string()
                }
            }
            SettingRow::Colorblind => settings.accessibility.colorblind.label().to_string(),
            SettingRow::TextScale => percent(settings.display.text_scale),
        }
    }

    /// Steps the row by `delta` of its coarse step (a keyboard press, or one
    /// repeat of a held click).
    pub fn adjust(self, settings: &mut Settings, delta: i32) {
        match self {
            SettingRow::MasterVolume => set_gain(&mut settings.audio.master, delta, VOLUME_STEP),
            SettingRow::MusicVolume => set_gain(&mut settings.audio.music, delta, VOLUME_STEP),
            SettingRow::SfxVolume => set_gain(&mut settings.audio.sfx, delta, VOLUME_STEP),
            SettingRow::ReducedMotion => {
                if delta != 0 {
                    settings.accessibility.reduced_motion = !settings.accessibility.reduced_motion;
                }
            }
            SettingRow::Colorblind => {
                settings.accessibility.colorblind =
                    settings.accessibility.colorblind.next(delta.signum());
            }
            SettingRow::TextScale => {
                let next = settings.display.text_scale + delta.signum() as f32 * TEXT_SCALE_STEP;
                settings.display.text_scale = next.clamp(TEXT_SCALE_MIN, TEXT_SCALE_MAX);
            }
        }
    }

    /// A single click's step. Volumes move finely so a tap is gentle; every other
    /// row steps as usual.
    pub fn adjust_tap(self, settings: &mut Settings, delta: i32) {
        match self {
            SettingRow::MasterVolume => {
                set_gain(&mut settings.audio.master, delta, VOLUME_TAP_STEP);
            }
            SettingRow::MusicVolume => {
                set_gain(&mut settings.audio.music, delta, VOLUME_TAP_STEP);
            }
            SettingRow::SfxVolume => {
                set_gain(&mut settings.audio.sfx, delta, VOLUME_TAP_STEP);
            }
            other => other.adjust(settings, delta),
        }
    }

    /// Whether the row is a volume, and so supports the click/hold stepper.
    pub fn is_volume(self) -> bool {
        matches!(
            self,
            SettingRow::MasterVolume | SettingRow::MusicVolume | SettingRow::SfxVolume
        )
    }
}

/// Moves a linear gain by `delta` steps of `step`, clamped to 0..=1.
fn set_gain(current: &mut f32, delta: i32, step: f32) {
    *current = (*current + delta.signum() as f32 * step).clamp(0.0, 1.0);
}

/// A linear gain rendered as a whole percent.
fn percent(gain: f32) -> String {
    format!("{}%", (gain.clamp(0.0, 1.0) * 100.0).round() as i32)
}

/// The schema from before accessibility, display and input were added: audio
/// only. Used to import a file written by an older build.
#[derive(Deserialize)]
struct SettingsV0 {
    #[allow(dead_code)]
    version: u32,
    audio: AudioSettings,
}

/// The schema from before keys were split into primary/secondary: a list of keys
/// per action. Used to import a v1 file.
#[derive(Deserialize)]
struct SettingsV1 {
    #[allow(dead_code)]
    version: u32,
    audio: AudioSettings,
    accessibility: AccessibilitySettings,
    display: DisplaySettings,
    input: InputMapV1,
}

/// The v1 input map: a list of key names per action.
#[derive(Deserialize)]
struct InputMapV1 {
    bindings: Vec<(Action, Vec<String>)>,
}

impl Settings {
    /// Serializes the payload.
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        bincode::serialize(self).map_err(|error| error.to_string())
    }

    /// Decodes a payload, importing an older schema where possible.
    ///
    /// The version is the first field, so it can be read straight off the bytes.
    /// A file from a newer build is refused (the player keeps defaults rather
    /// than a half-understood mix); an older one is migrated; anything
    /// unreadable falls back to defaults rather than failing.
    pub fn decode(bytes: &[u8]) -> Settings {
        match peek_version(bytes) {
            Some(version) if version == SETTINGS_VERSION => bincode::deserialize::<Settings>(bytes)
                .unwrap_or_else(|error| {
                    warn!("settings were unreadable ({error}); using defaults");
                    Settings::default()
                }),
            Some(version) if version < SETTINGS_VERSION => migrate(version, bytes),
            Some(version) => {
                warn!("settings are from a newer build (v{version}); using defaults");
                Settings::default()
            }
            None => {
                warn!("settings were empty or truncated; using defaults");
                Settings::default()
            }
        }
    }
}

/// The version stored in the first four bytes (bincode's fixint little-endian
/// `u32`), if the buffer is long enough to hold one.
fn peek_version(bytes: &[u8]) -> Option<u32> {
    let prefix: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
    Some(u32::from_le_bytes(prefix))
}

/// Imports an older payload into the current schema, defaulting fields that did
/// not exist yet. A payload that cannot be read at all yields defaults.
fn migrate(version: u32, bytes: &[u8]) -> Settings {
    match version {
        0 => {
            let options = bincode::DefaultOptions::new()
                .with_fixint_encoding()
                .allow_trailing_bytes();
            match options.deserialize::<SettingsV0>(bytes) {
                Ok(old) => {
                    warn!("imported settings from v0; new options use their defaults");
                    Settings {
                        audio: old.audio,
                        ..Settings::default()
                    }
                }
                Err(error) => {
                    warn!("could not import v0 settings ({error}); using defaults");
                    Settings::default()
                }
            }
        }
        1 => match bincode::deserialize::<SettingsV1>(bytes) {
            Ok(old) => {
                warn!("imported settings from v1; keys keep their first two bindings");
                Settings {
                    audio: old.audio,
                    accessibility: old.accessibility,
                    display: old.display,
                    input: InputMap::from_v1(old.input.bindings),
                    ..Settings::default()
                }
            }
            Err(error) => {
                warn!("could not import v1 settings ({error}); using defaults");
                Settings::default()
            }
        },
        _ => {
            warn!("no migration from settings v{version}; using defaults");
            Settings::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_neutral_and_silent_free() {
        let settings = Settings::default();
        assert_eq!(settings.version, SETTINGS_VERSION);
        assert_eq!(settings.audio, AudioSettings::default());
        assert!(!settings.accessibility.reduced_motion);
        assert_eq!(settings.accessibility.colorblind, ColorblindMode::Off);
        assert_eq!(settings.display.text_scale, 1.0);
        assert_eq!(
            settings.input.binding(Action::MoveUp, KeySlot::Primary),
            Some("KeyW")
        );
    }

    #[test]
    fn encode_then_decode_round_trips() {
        let mut original = Settings::default();
        original.audio.master = 0.4;
        original.accessibility.reduced_motion = true;
        original.accessibility.colorblind = ColorblindMode::Tritanopia;
        original.display.text_scale = 1.25;

        let bytes = original.encode().unwrap();
        assert_eq!(Settings::decode(&bytes), original);
    }

    #[test]
    fn volumes_step_in_tenths_and_clamp() {
        let mut settings = Settings::default();
        SettingRow::MasterVolume.adjust(&mut settings, 1);
        assert_eq!(settings.audio.master, 1.0, "clamps at full");
        SettingRow::SfxVolume.adjust(&mut settings, -1);
        assert!((settings.audio.sfx - 0.9).abs() < 1e-6);
        for _ in 0..20 {
            SettingRow::SfxVolume.adjust(&mut settings, -1);
        }
        assert_eq!(settings.audio.sfx, 0.0, "clamps at silence");
    }

    #[test]
    fn reduced_motion_toggles_and_colorblind_cycles() {
        let mut settings = Settings::default();
        SettingRow::ReducedMotion.adjust(&mut settings, 1);
        assert!(settings.accessibility.reduced_motion);
        SettingRow::ReducedMotion.adjust(&mut settings, 1);
        assert!(!settings.accessibility.reduced_motion);

        SettingRow::Colorblind.adjust(&mut settings, 1);
        assert_eq!(
            settings.accessibility.colorblind,
            ColorblindMode::Protanopia
        );
        SettingRow::Colorblind.adjust(&mut settings, -1);
        assert_eq!(settings.accessibility.colorblind, ColorblindMode::Off);
        SettingRow::Colorblind.adjust(&mut settings, -1);
        assert_eq!(
            settings.accessibility.colorblind,
            ColorblindMode::Tritanopia,
            "cycles backwards from the first entry"
        );
    }

    #[test]
    fn text_scale_steps_and_clamps() {
        let mut settings = Settings::default();
        SettingRow::TextScale.adjust(&mut settings, 1);
        assert!((settings.display.text_scale - 1.25).abs() < 1e-6);
        SettingRow::TextScale.adjust(&mut settings, 1);
        assert!((settings.display.text_scale - 1.5).abs() < 1e-6);
        SettingRow::TextScale.adjust(&mut settings, 1);
        assert!((settings.display.text_scale - 1.5).abs() < 1e-6, "clamps");
        SettingRow::TextScale.adjust(&mut settings, -1);
        assert!((settings.display.text_scale - 1.25).abs() < 1e-6);
    }

    #[test]
    fn row_values_read_back_the_settings() {
        let mut settings = Settings::default();
        settings.audio.music = 0.3;
        assert_eq!(SettingRow::MusicVolume.value(&settings), "30%");
        assert_eq!(SettingRow::ReducedMotion.value(&settings), "Off");
        assert_eq!(SettingRow::Colorblind.value(&settings), "Off");
        assert_eq!(SettingRow::TextScale.value(&settings), "100%");
    }

    #[test]
    fn a_v0_file_is_imported_and_the_rest_defaulted() {
        // An older build only knew about audio.
        #[derive(Serialize)]
        struct Legacy {
            version: u32,
            audio: AudioSettings,
        }
        let legacy = Legacy {
            version: 0,
            audio: AudioSettings {
                master: 0.5,
                music: 0.2,
                sfx: 0.9,
            },
        };
        let bytes = bincode::serialize(&legacy).unwrap();

        let migrated = Settings::decode(&bytes);
        assert_eq!(
            migrated.audio,
            AudioSettings {
                master: 0.5,
                music: 0.2,
                sfx: 0.9,
            },
            "the old audio mix is imported"
        );
        assert_eq!(migrated.version, SETTINGS_VERSION);
        assert!(!migrated.accessibility.reduced_motion, "new fields default");
        assert_eq!(migrated.display.text_scale, 1.0);
    }

    #[test]
    fn a_newer_version_is_refused() {
        let settings = Settings {
            version: SETTINGS_VERSION + 1,
            ..Settings::default()
        };
        let bytes = bincode::serialize(&settings).unwrap();
        assert_eq!(
            Settings::decode(&bytes),
            Settings::default(),
            "a future payload falls back to defaults"
        );
    }

    #[test]
    fn garbage_bytes_fall_back_to_defaults() {
        assert_eq!(Settings::decode(&[]), Settings::default());
        assert_eq!(Settings::decode(&[1, 2]), Settings::default());
    }

    #[test]
    fn input_map_defaults_match_the_shipped_scheme() {
        let map = InputMap::default();
        assert!(map.is_bound(Action::MoveUp, KeyCode::KeyW));
        assert!(map.is_bound(Action::MoveUp, KeyCode::ArrowUp));
        assert!(map.is_bound(Action::Interact, KeyCode::Space));
        assert!(map.is_bound(Action::Pause, KeyCode::Escape));
        assert!(!map.is_bound(Action::Interact, KeyCode::KeyQ));
        assert_eq!(
            map.binding(Action::MoveUp, KeySlot::Primary),
            Some("KeyW"),
            "W is the primary"
        );
        assert_eq!(
            map.binding(Action::MoveUp, KeySlot::Secondary),
            Some("ArrowUp"),
            "Up is the secondary"
        );
    }

    #[test]
    fn binding_sets_one_slot_without_touching_the_other() {
        let mut map = InputMap::default();
        map.set(Action::MoveUp, KeySlot::Secondary, "KeyJ".to_string());
        assert_eq!(map.binding(Action::MoveUp, KeySlot::Primary), Some("KeyW"));
        assert_eq!(
            map.binding(Action::MoveUp, KeySlot::Secondary),
            Some("KeyJ")
        );
        assert!(map.is_bound(Action::MoveUp, KeyCode::KeyJ));
    }

    #[test]
    fn binding_a_key_steals_it_from_every_other_slot() {
        let mut map = InputMap::default();
        // KeyW is MoveUp's primary; hand it to MoveDown's secondary.
        map.set(Action::MoveDown, KeySlot::Secondary, "KeyW".to_string());
        assert_eq!(map.binding(Action::MoveUp, KeySlot::Primary), None);
        assert_eq!(
            map.binding(Action::MoveDown, KeySlot::Secondary),
            Some("KeyW")
        );
    }

    #[test]
    fn resetting_restores_the_shipped_bindings() {
        let mut map = InputMap::default();
        map.set(Action::MoveUp, KeySlot::Primary, "KeyJ".to_string());
        map.clear(Action::Interact, KeySlot::Primary);
        map.reset_defaults();
        assert_eq!(map, InputMap::default());
        assert_eq!(map.binding(Action::MoveUp, KeySlot::Primary), Some("KeyW"));
        assert_eq!(
            map.binding(Action::Interact, KeySlot::Primary),
            Some("Space")
        );
    }

    #[test]
    fn a_v1_file_imports_its_keys_as_primary_and_secondary() {
        #[derive(Serialize)]
        struct LegacyInput {
            bindings: Vec<(Action, Vec<String>)>,
        }
        #[derive(Serialize)]
        struct Legacy {
            version: u32,
            audio: AudioSettings,
            accessibility: AccessibilitySettings,
            display: DisplaySettings,
            input: LegacyInput,
        }
        let legacy = Legacy {
            version: 1,
            audio: AudioSettings {
                master: 0.7,
                music: 0.4,
                sfx: 0.2,
            },
            accessibility: AccessibilitySettings {
                reduced_motion: true,
                colorblind: ColorblindMode::Protanopia,
            },
            display: DisplaySettings { text_scale: 1.25 },
            input: LegacyInput {
                bindings: vec![(Action::MoveUp, vec!["KeyJ".to_string(), "KeyL".to_string()])],
            },
        };
        let bytes = bincode::serialize(&legacy).unwrap();

        let migrated = Settings::decode(&bytes);
        assert_eq!(migrated.version, SETTINGS_VERSION);
        assert_eq!(migrated.audio.master, 0.7, "audio is imported");
        assert!(migrated.accessibility.reduced_motion);
        assert_eq!(migrated.display.text_scale, 1.25);
        assert_eq!(
            migrated.input.binding(Action::MoveUp, KeySlot::Primary),
            Some("KeyJ")
        );
        assert_eq!(
            migrated.input.binding(Action::MoveUp, KeySlot::Secondary),
            Some("KeyL")
        );
        assert_eq!(
            migrated.input.binding(Action::Interact, KeySlot::Primary),
            Some("Space"),
            "actions missing from the old map keep their shipped binding"
        );
    }
}
