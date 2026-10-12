//! The save payload and the resource that reads and writes it.
//!
//! Everything a run needs to survive a quit is gathered into [`SaveData`], a
//! plain serde struct, then handed to a [`SaveBackend`]. The individual
//! resources stay the game's source of truth; this module is only the boundary
//! where they are copied in and out.

use crate::resources::boss_progress::BossProgress;
use crate::resources::difficulty::Difficulty;
use crate::resources::farm::{CropUnlocks, DayCounter, FarmState};
use crate::resources::goals::GoalsProgress;
use crate::resources::inventory::Inventory;
use crate::resources::run_data::PlayerGear;
use crate::resources::settings::{SETTINGS_KEY, Settings};
use crate::utils::save_backend::{SaveBackend, SaveError, platform_backend};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The key the one run slot lives under. Single-slot for the MVP.
pub const SAVE_KEY: &str = "run";

/// Bumped when the payload's shape changes. Old saves are discarded rather than
/// migrated, which is fine while the schema is still moving.
pub const SAVE_VERSION: u32 = 2;

/// A complete run, ready to be serialized.
///
/// The field set is exactly what a resume needs. Transient session state
/// (the current day phase, panel open flags, the fade) is deliberately absent:
/// a load always resumes into a fresh farming day. The one exception is
/// [`Self::in_bossfight`], which is a deliberate breadcrumb rather than state.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SaveData {
    pub version: u32,
    pub day: u32,
    pub gear: PlayerGear,
    pub inventory: Inventory,
    pub farm: FarmState,
    pub unlocks: CropUnlocks,
    pub bosses: BossProgress,
    pub goals: GoalsProgress,
    pub difficulty: Difficulty,
    /// True when the save was written while a boss fight was live. A Permadeath
    /// save that still carries this on load was force-quit mid-fight and is
    /// invalidated (see the persistence plugin).
    pub in_bossfight: bool,
}

impl SaveData {
    /// Copies the run resources into a payload. Takes references so the caller
    /// keeps ownership of the live resources.
    #[allow(clippy::too_many_arguments)]
    pub fn capture(
        day: &DayCounter,
        gear: &PlayerGear,
        inventory: &Inventory,
        farm: &FarmState,
        unlocks: &CropUnlocks,
        bosses: &BossProgress,
        goals: &GoalsProgress,
        difficulty: Difficulty,
        in_bossfight: bool,
    ) -> Self {
        Self {
            version: SAVE_VERSION,
            day: day.0,
            gear: gear.clone(),
            inventory: inventory.clone(),
            farm: farm.clone(),
            unlocks: unlocks.clone(),
            bosses: bosses.clone(),
            goals: goals.clone(),
            difficulty,
            in_bossfight,
        }
    }
}

/// Reads and writes the run slot through the platform's backend.
///
/// The backend is boxed behind the [`SaveBackend`] trait so tests can swap in a
/// [`crate::utils::save_backend::MemoryBackend`] with [`SaveManager::with_backend`].
#[derive(Resource)]
pub struct SaveManager {
    backend: Box<dyn SaveBackend>,
}

impl Default for SaveManager {
    fn default() -> Self {
        Self {
            backend: platform_backend(),
        }
    }
}

impl SaveManager {
    /// A manager over an explicit backend. Tests use this to stay off disk.
    pub fn with_backend(backend: Box<dyn SaveBackend>) -> Self {
        Self { backend }
    }

    /// Serializes and stores the run. Replaces any previous save.
    pub fn save(&self, data: &SaveData) -> Result<(), SaveError> {
        let bytes =
            bincode::serialize(data).map_err(|error| SaveError::Encode(error.to_string()))?;
        self.backend.save(SAVE_KEY, &bytes)
    }

    /// Loads the run, if one is stored under a version this build understands.
    pub fn load(&self) -> Result<SaveData, SaveError> {
        let bytes = self.backend.load(SAVE_KEY)?;
        let data: SaveData =
            bincode::deserialize(&bytes).map_err(|error| SaveError::Encode(error.to_string()))?;
        if data.version != SAVE_VERSION {
            return Err(SaveError::Encode(format!(
                "save version {} is not {}",
                data.version, SAVE_VERSION
            )));
        }
        Ok(data)
    }

    /// Whether a resumable save exists.
    pub fn has_save(&self) -> bool {
        self.backend.exists(SAVE_KEY)
    }

    /// Removes the run slot. Ignored if nothing is stored.
    pub fn delete(&self) -> Result<(), SaveError> {
        self.backend.delete(SAVE_KEY)
    }

    /// Serializes and stores the player's settings in their own slot, separate
    /// from the run.
    pub fn save_settings(&self, settings: &Settings) -> Result<(), SaveError> {
        let bytes = settings.encode().map_err(SaveError::Encode)?;
        self.backend.save(SETTINGS_KEY, &bytes)
    }

    /// Loads the player's settings. A missing or unreadable slot yields the
    /// defaults, so the game always has a usable set of preferences.
    pub fn load_settings(&self) -> Settings {
        match self.backend.load(SETTINGS_KEY) {
            Ok(bytes) => Settings::decode(&bytes),
            Err(_) => Settings::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::boss::BossId;
    use crate::components::gear::{GearPiece, GearSet, GearSlot};
    use crate::components::pot::{CropType, Pot, PotState};
    use crate::utils::save_backend::MemoryBackend;

    fn sample() -> SaveData {
        let mut gear = PlayerGear::default();
        gear.own(GearPiece::new(GearSet::Starter, GearSlot::Weapon));
        gear.equip(&GearPiece::new(GearSet::Starter, GearSlot::Weapon));

        let mut inventory = Inventory::default();
        inventory.add_crop(CropType::Starter, 4);
        inventory.add_material(crate::components::gear::MaterialType::BossA1, 2);

        let mut farm = FarmState::default();
        let mut pot = Pot::new(0);
        pot.plant(CropType::CropA);
        farm.pots.push(pot);

        let mut bosses = BossProgress::default();
        bosses.record(BossId::BossA);

        let mut goals = GoalsProgress::default();
        goals.complete(crate::resources::goals::GoalId::PlantStarter);

        SaveData::capture(
            &DayCounter(5),
            &gear,
            &inventory,
            &farm,
            &CropUnlocks::new(),
            &bosses,
            &goals,
            Difficulty::Hard,
            false,
        )
    }

    fn manager() -> SaveManager {
        SaveManager::with_backend(Box::new(MemoryBackend::default()))
    }

    #[test]
    fn capture_takes_the_whole_run() {
        let data = sample();
        assert_eq!(data.version, SAVE_VERSION);
        assert_eq!(data.day, 5);
        assert_eq!(data.difficulty, Difficulty::Hard);
        assert!(data.bosses.boss_a);
        assert_eq!(data.inventory.crop_count(CropType::Starter), 4);
        assert_eq!(data.farm.pots.len(), 1);
        assert_eq!(data.farm.pots[0].state, PotState::Planted);
        assert!(
            data.gear
                .owns(&GearPiece::new(GearSet::Starter, GearSlot::Weapon))
        );
    }

    #[test]
    fn save_then_load_round_trips_every_field() {
        let manager = manager();
        let original = sample();
        manager.save(&original).unwrap();

        let loaded = manager.load().unwrap();
        assert_eq!(loaded, original);
    }

    #[test]
    fn has_save_follows_the_backend() {
        let manager = manager();
        assert!(!manager.has_save());
        manager.save(&sample()).unwrap();
        assert!(manager.has_save());
        manager.delete().unwrap();
        assert!(!manager.has_save());
    }

    #[test]
    fn loading_without_a_save_is_not_found() {
        let manager = manager();
        assert!(matches!(manager.load(), Err(SaveError::NotFound)));
    }

    #[test]
    fn a_future_version_is_refused() {
        let manager = manager();
        let mut data = sample();
        data.version = SAVE_VERSION + 1;
        // Write the bytes straight through so the version check is what runs.
        let bytes = bincode::serialize(&data).unwrap();
        manager.backend.save(SAVE_KEY, &bytes).unwrap();

        assert!(matches!(manager.load(), Err(SaveError::Encode(_))));
    }

    #[test]
    fn settings_live_in_their_own_slot() {
        let manager = manager();
        let mut settings = Settings::default();
        settings.audio.music = 0.2;
        settings.display.text_scale = 1.25;
        manager.save_settings(&settings).unwrap();

        assert_eq!(manager.load_settings(), settings);
        assert!(
            !manager.has_save(),
            "saving settings does not create a run save"
        );

        manager.save(&sample()).unwrap();
        assert_eq!(
            manager.load_settings(),
            settings,
            "writing a run leaves the settings untouched"
        );
    }

    #[test]
    fn loading_settings_without_a_slot_yields_defaults() {
        let manager = manager();
        assert_eq!(manager.load_settings(), Settings::default());
    }
}
