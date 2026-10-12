//! Player settings: audio mix, accessibility and display preferences.
//!
//! Settings live in their own save slot ([`SETTINGS_KEY`]) rather than in the run
//! save, so they survive New Game, death and every run. The payload carries a
//! [`SETTINGS_VERSION`]; when a build changes the shape of [`Settings`], loading
//! an older file still imports what it can and defaults the rest (see
//! [`Settings::decode`]) instead of throwing the player's preferences away.

use crate::resources::audio_settings::AudioSettings;
use bevy::prelude::*;
use bincode::Options;
use log::warn;
use serde::{Deserialize, Serialize};

/// The save slot settings live under. Separate from the run's `"run"` slot.
pub const SETTINGS_KEY: &str = "settings";

/// Bumped when the settings payload's shape changes. Older files are migrated.
pub const SETTINGS_VERSION: u32 = 1;

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

/// Key bindings per action, stored as stable key names (a `KeyCode`'s debug
/// name). Names, not `KeyCode`s, so the payload never depends on Bevy's own
/// serialization feature. The default bindings match the game's shipped scheme.
#[derive(Reflect, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputMap {
    bindings: Vec<(Action, Vec<String>)>,
}

impl Default for InputMap {
    fn default() -> Self {
        Self {
            bindings: Action::ALL
                .iter()
                .map(|action| (*action, default_bindings(*action)))
                .collect(),
        }
    }
}

impl InputMap {
    /// The key names bound to `action` (empty if unknown).
    pub fn binding_names(&self, action: Action) -> &[String] {
        self.bindings
            .iter()
            .find(|(bound, _)| *bound == action)
            .map(|(_, names)| names.as_slice())
            .unwrap_or(&[])
    }

    /// Replaces an action's bindings.
    pub fn rebind(&mut self, action: Action, names: Vec<String>) {
        match self.bindings.iter_mut().find(|(bound, _)| *bound == action) {
            Some((_, slot)) => *slot = names,
            None => self.bindings.push((action, names)),
        }
    }

    /// Whether `key` is bound to `action`.
    pub fn is_bound(&self, action: Action, key: KeyCode) -> bool {
        let name = format!("{key:?}");
        self.binding_names(action).contains(&name)
    }
}

/// The shipped bindings for one action, as key names.
fn default_bindings(action: Action) -> Vec<String> {
    let names: &[&str] = match action {
        Action::MoveUp => &["KeyW", "ArrowUp"],
        Action::MoveDown => &["KeyS", "ArrowDown"],
        Action::MoveLeft => &["KeyA", "ArrowLeft"],
        Action::MoveRight => &["KeyD", "ArrowRight"],
        Action::Interact => &["Space"],
        Action::LightAttack => &["KeyQ"],
        Action::HeavyAttack => &["KeyE"],
        Action::OpenInventory => &["KeyI"],
        Action::OpenCrafting => &["Tab", "KeyC"],
        Action::Pause => &["Escape"],
    };
    names.iter().map(|name| name.to_string()).collect()
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
    /// Key bindings. Kept for a future rebinding UI; not yet read by gameplay.
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

/// Every steppable row, in display order.
pub const SETTING_ROWS: [SettingRow; 6] = [
    SettingRow::MasterVolume,
    SettingRow::MusicVolume,
    SettingRow::SfxVolume,
    SettingRow::ReducedMotion,
    SettingRow::Colorblind,
    SettingRow::TextScale,
];

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

    /// Steps the row by `delta` (a mouse click uses +1).
    pub fn adjust(self, settings: &mut Settings, delta: i32) {
        let step = delta.signum();
        match self {
            SettingRow::MasterVolume => {
                settings.audio.master = step_gain(settings.audio.master, step);
            }
            SettingRow::MusicVolume => {
                settings.audio.music = step_gain(settings.audio.music, step);
            }
            SettingRow::SfxVolume => {
                settings.audio.sfx = step_gain(settings.audio.sfx, step);
            }
            SettingRow::ReducedMotion => {
                if step != 0 {
                    settings.accessibility.reduced_motion = !settings.accessibility.reduced_motion;
                }
            }
            SettingRow::Colorblind => {
                settings.accessibility.colorblind = settings.accessibility.colorblind.next(step);
            }
            SettingRow::TextScale => {
                let next = settings.display.text_scale + step as f32 * TEXT_SCALE_STEP;
                settings.display.text_scale = next.clamp(TEXT_SCALE_MIN, TEXT_SCALE_MAX);
            }
        }
    }
}

/// Volumes step in 10% increments and clamp to 0..=1.
fn step_gain(current: f32, step: i32) -> f32 {
    (current + step as f32 * 0.1).clamp(0.0, 1.0)
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
        assert_eq!(settings.input.binding_names(Action::MoveUp).len(), 2);
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
    }

    #[test]
    fn rebinding_replaces_an_actions_keys() {
        let mut map = InputMap::default();
        map.rebind(Action::LightAttack, vec!["KeyJ".to_string()]);
        assert!(!map.is_bound(Action::LightAttack, KeyCode::KeyQ));
        assert!(map.is_bound(Action::LightAttack, KeyCode::KeyJ));
    }
}
