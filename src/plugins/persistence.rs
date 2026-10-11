//! Wiring for save and load.
//!
//! [`SaveManager`] knows how to serialize a run; this plugin decides *when*.
//! There are two automatic saves:
//!
//! * **on progress** — the events that mark real advancement (`DayAdvanced`,
//!   `BossDefeated`, `GearCrafted`, `PlayerDied`) write a fresh save mid-run, so
//!   a closed tab resumes close to where it left off;
//! * **on leaving play** — exiting [`GameState::Playing`] writes once more. This
//!   is what catches `Return to Main Menu` from the pause and victory screens,
//!   and it runs at the state transition, before the title screen appears.
//!
//! Loading happens through [`ContinueRequested`], emitted by the title screen's
//! `Continue` button. The loaded payload is written back into the run resources
//! and play begins on a fresh farming day.

use crate::events::{DayAdvanced, GearCrafted, PlayerDied};
use crate::resources::boss_progress::BossProgress;
use crate::resources::difficulty::Difficulty;
use crate::resources::farm::{CropUnlocks, DayCounter, FarmState};
use crate::resources::goals::GoalsProgress;
use crate::resources::inventory::Inventory;
use crate::resources::main_menu::{ContinueRequested, MenuFeatureFlags, NewGameRequested};
use crate::resources::run_data::PlayerGear;
use crate::resources::save_manager::{SaveData, SaveManager};
use crate::states::GameState;
use crate::systems::scene_fade::SceneFade;
use bevy::ecs::message::MessageReader;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use log::{info, warn};

pub struct PersistencePlugin;

impl Plugin for PersistencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SaveManager>()
            .init_resource::<MenuFeatureFlags>()
            .add_message::<ContinueRequested>()
            .add_message::<NewGameRequested>()
            // At boot, reflect whatever save already exists so the title screen's
            // `Continue` slot is revealed before it is spawned.
            .add_systems(OnEnter(GameState::LoadingAssets), refresh_continue_flag)
            .add_systems(
                Update,
                autosave_on_progress.run_if(in_state(GameState::Playing)),
            )
            // Leaving play saves once, then refreshes the flag so `Continue` is up
            // to date by the time the title screen spawns.
            .add_systems(
                OnExit(GameState::Playing),
                (autosave_on_exit, refresh_continue_flag).chain(),
            )
            .add_systems(Update, (clear_save_on_new_game, handle_continue).chain());
    }
}

/// The run resources, bundled so the save/load systems keep readable signatures.
#[derive(SystemParam)]
pub struct RunState<'w> {
    pub day: ResMut<'w, DayCounter>,
    pub gear: ResMut<'w, PlayerGear>,
    pub inventory: ResMut<'w, Inventory>,
    pub farm: ResMut<'w, FarmState>,
    pub unlocks: ResMut<'w, CropUnlocks>,
    pub bosses: ResMut<'w, BossProgress>,
    pub goals: ResMut<'w, GoalsProgress>,
    pub difficulty: ResMut<'w, Difficulty>,
}

impl RunState<'_> {
    /// Copies the live run into a serializable payload.
    pub fn capture(&self) -> SaveData {
        SaveData::capture(
            &self.day,
            &self.gear,
            &self.inventory,
            &self.farm,
            &self.unlocks,
            &self.bosses,
            &self.goals,
            *self.difficulty,
        )
    }

    /// Restores a payload over the live run.
    pub fn apply(&mut self, data: SaveData) {
        self.day.0 = data.day;
        *self.gear = data.gear;
        *self.inventory = data.inventory;
        *self.farm = data.farm;
        *self.unlocks = data.unlocks;
        *self.bosses = data.bosses;
        *self.goals = data.goals;
        *self.difficulty = data.difficulty;
    }
}

/// Writes the run, logging rather than failing: a save error should never
/// interrupt play.
fn write_save(manager: &SaveManager, run: &RunState) {
    match manager.save(&run.capture()) {
        Ok(()) => info!("saved run (day {})", run.day.0),
        Err(error) => warn!("could not save run: {error}"),
    }
}

/// Saves whenever the run made measurable progress.
fn autosave_on_progress(
    manager: Res<SaveManager>,
    run: RunState,
    mut day_advanced: MessageReader<DayAdvanced>,
    mut bosses: MessageReader<crate::events::BossDefeated>,
    mut crafted: MessageReader<GearCrafted>,
    mut died: MessageReader<PlayerDied>,
) {
    let progressed = day_advanced.read().count()
        + bosses.read().count()
        + crafted.read().count()
        + died.read().count()
        > 0;
    if progressed {
        write_save(&manager, &run);
    }
}

/// The catch-all save for leaving play, which includes returning to the menu.
fn autosave_on_exit(manager: Res<SaveManager>, run: RunState) {
    write_save(&manager, &run);
}

/// Reveals or hides the title screen's `Continue` slot to match the save slot.
fn refresh_continue_flag(manager: Res<SaveManager>, mut flags: ResMut<MenuFeatureFlags>) {
    flags.load_game = manager.has_save();
}

/// Starting a new game drops the previous run's save, so `Continue` cannot
/// resurrect it before the new run has saved anything of its own.
fn clear_save_on_new_game(
    manager: Res<SaveManager>,
    mut requests: MessageReader<NewGameRequested>,
    mut flags: ResMut<MenuFeatureFlags>,
) {
    if requests.read().count() == 0 {
        return;
    }
    if let Err(error) = manager.delete() {
        warn!("could not clear old save: {error}");
    }
    flags.load_game = false;
}

/// Loads the saved run and begins play when the title screen asks to continue.
fn handle_continue(
    manager: Res<SaveManager>,
    mut run: RunState,
    mut requests: MessageReader<ContinueRequested>,
    mut fade: ResMut<SceneFade>,
) {
    if requests.read().count() == 0 {
        return;
    }
    match manager.load() {
        Ok(data) => {
            run.apply(data);
            info!("continuing saved run (day {})", run.day.0);
            fade.begin_game(GameState::Playing);
        }
        Err(error) => warn!("could not load run: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::gear::{GearPiece, GearSet, GearSlot};
    use crate::components::pot::CropType;
    use crate::resources::boss_progress::BossProgress;
    use crate::resources::save_manager::SAVE_VERSION;
    use crate::systems::scene_fade::SceneFadePlugin;
    use crate::utils::save_backend::MemoryBackend;
    use bevy::state::app::StatesPlugin;

    fn setup() -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            SceneFadePlugin,
            PersistencePlugin,
        ))
        .init_state::<GameState>()
        .init_resource::<DayCounter>()
        .init_resource::<PlayerGear>()
        .init_resource::<Inventory>()
        .init_resource::<FarmState>()
        .init_resource::<CropUnlocks>()
        .init_resource::<BossProgress>()
        .init_resource::<GoalsProgress>()
        .init_resource::<Difficulty>()
        .add_message::<DayAdvanced>()
        .add_message::<crate::events::BossDefeated>()
        .add_message::<GearCrafted>()
        .add_message::<PlayerDied>();
        // Keep tests off disk.
        app.insert_resource(SaveManager::with_backend(
            Box::new(MemoryBackend::default()),
        ));
        app
    }

    fn enter_playing(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
    }

    fn enter_menu(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::MainMenu);
        app.update();
    }

    fn flags(app: &App) -> MenuFeatureFlags {
        *app.world().resource::<MenuFeatureFlags>()
    }

    #[test]
    fn boot_reflects_an_existing_save() {
        let mut app = setup();
        // No save yet.
        app.update();
        assert!(!flags(&app).load_game);

        // Move off boot, seed a save, then boot again.
        enter_menu(&mut app);
        app.world()
            .resource::<SaveManager>()
            .save(&SaveData::capture(
                &DayCounter(3),
                &PlayerGear::default(),
                &Inventory::default(),
                &FarmState::default(),
                &CropUnlocks::new(),
                &BossProgress::default(),
                &GoalsProgress::default(),
                Difficulty::Normal,
            ))
            .unwrap();

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::LoadingAssets);
        app.update();
        assert!(flags(&app).load_game);
    }

    #[test]
    fn progress_events_write_a_save() {
        let mut app = setup();
        enter_playing(&mut app);
        app.world_mut().resource_mut::<DayCounter>().0 = 7;

        app.world_mut().write_message(DayAdvanced { day: 7 });
        app.update();
        // A second update lets the message be seen if it was queued after the
        // reader ran.
        app.update();

        let loaded = app.world().resource::<SaveManager>().load().unwrap();
        assert_eq!(loaded.day, 7);
    }

    #[test]
    fn leaving_play_saves_and_reveals_continue() {
        let mut app = setup();
        enter_playing(&mut app);
        app.world_mut().resource_mut::<DayCounter>().0 = 4;

        enter_menu(&mut app);

        assert!(
            flags(&app).load_game,
            "continue is offered after a run saves"
        );
        assert_eq!(
            app.world().resource::<SaveManager>().load().unwrap().day,
            4,
            "leaving play saved the run"
        );
    }

    #[test]
    fn new_game_drops_the_previous_save() {
        let mut app = setup();
        enter_playing(&mut app);
        enter_menu(&mut app);
        assert!(flags(&app).load_game);

        app.world_mut().write_message(NewGameRequested);
        app.update();

        assert!(!flags(&app).load_game);
        assert!(!app.world().resource::<SaveManager>().has_save());
    }

    #[test]
    fn continue_restores_the_run_and_enters_playing() {
        let mut app = setup();
        // Seed a save with meaningful state.
        let mut gear = PlayerGear::default();
        gear.own(GearPiece::new(GearSet::Master, GearSlot::Weapon));
        let mut inventory = Inventory::default();
        inventory.add_crop(CropType::CropA, 9);
        let data = SaveData {
            version: SAVE_VERSION,
            day: 11,
            gear,
            inventory,
            farm: FarmState::default(),
            unlocks: CropUnlocks::new(),
            bosses: BossProgress::default(),
            goals: GoalsProgress::default(),
            difficulty: Difficulty::Apocalypse,
        };
        app.world().resource::<SaveManager>().save(&data).unwrap();

        app.world_mut().write_message(ContinueRequested);
        for _ in 0..3 {
            app.update();
        }

        assert_eq!(
            app.world().resource::<State<GameState>>().get(),
            &GameState::Playing
        );
        assert_eq!(app.world().resource::<DayCounter>().0, 11);
        assert_eq!(
            app.world().resource::<Difficulty>(),
            &Difficulty::Apocalypse
        );
        assert_eq!(
            app.world()
                .resource::<Inventory>()
                .crop_count(CropType::CropA),
            9
        );
    }

    #[test]
    fn continue_without_a_save_stays_on_the_menu() {
        let mut app = setup();
        app.world_mut().write_message(ContinueRequested);
        app.update();
        app.update();

        assert_ne!(
            app.world().resource::<State<GameState>>().get(),
            &GameState::Playing
        );
    }

    #[test]
    fn memory_backend_round_trips_through_a_capture() {
        // Guards the on-progress path end to end without an App.
        let backend = MemoryBackend::default();
        let manager = SaveManager::with_backend(Box::new(backend));
        let data = SaveData::capture(
            &DayCounter(2),
            &PlayerGear::default(),
            &Inventory::default(),
            &FarmState::default(),
            &CropUnlocks::new(),
            &BossProgress::default(),
            &GoalsProgress::default(),
            Difficulty::Normal,
        );
        manager.save(&data).unwrap();
        assert_eq!(manager.load().unwrap(), data);
    }
}
