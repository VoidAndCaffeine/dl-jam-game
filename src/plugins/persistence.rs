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
//!
//! # Permadeath and the bossfight breadcrumb
//!
//! A Permadeath run can be force-quit (closed tab, killed process) mid-fight to
//! dodge a death that would have ended it. To catch that, entering a boss fight
//! in Permadeath writes [`InBossfight`] into the save; leaving the fight — win,
//! loss, or a walk back to the farm — clears it. A save that is *still* marked
//! when it is loaded therefore means the player quit mid-fight, and its
//! Permadeath run is invalidated: the difficulty steps down to Apocalypse and
//! the player is told, with a single button to acknowledge.

use crate::events::{BossDefeated, CropHarvested, DayAdvanced, GearCrafted, PlayerDied};
use crate::plugins::day_cycle::VictoryMode;
use crate::resources::boss_progress::BossProgress;
use crate::resources::confirm::{ConfirmDialog, ConfirmKind, ConfirmResolved};
use crate::resources::day_cycle::{DayCycle, InBossfight, ResumeRun, RunOver};
use crate::resources::difficulty::Difficulty;
use crate::resources::farm::{CropUnlocks, DayCounter, FarmState};
use crate::resources::goals::GoalsProgress;
use crate::resources::inventory::Inventory;
use crate::resources::main_menu::{ContinueRequested, MenuFeatureFlags, NewGameRequested};
use crate::resources::points::RunPoints;
use crate::resources::run_data::PlayerGear;
use crate::resources::save_manager::{SaveData, SaveManager};
use crate::states::{DayPhase, GameState};
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
            // Persistence writes these on load; init them here so it stands alone.
            .init_resource::<ResumeRun>()
            .init_resource::<InBossfight>()
            .init_resource::<RunOver>()
            .init_resource::<DayCycle>()
            .init_resource::<ConfirmDialog>()
            .init_resource::<VictoryMode>()
            .init_resource::<RunPoints>()
            .add_message::<ContinueRequested>()
            .add_message::<NewGameRequested>()
            .add_message::<ConfirmResolved>()
            // At boot, reflect whatever save already exists so the title screen's
            // `Continue` slot is revealed before it is spawned.
            .add_systems(OnEnter(GameState::LoadingAssets), refresh_continue_flag)
            // The authoritative close of a Permadeath run. The death message also
            // closes it, but message/order timing is not guaranteed, whereas the
            // run-over screen's mode is set before this state is entered.
            .add_systems(OnEnter(GameState::Victory), close_ended_run)
            // The bossfight breadcrumb, written into the save while a fight is up.
            .add_systems(OnEnter(DayPhase::BossFight), mark_bossfight)
            .add_systems(OnEnter(DayPhase::Farming), clear_bossfight_on_farm)
            .add_systems(
                Update,
                // Clear the breadcrumb and close an ended Permadeath run before
                // the progress save, so neither is written as still-live.
                (
                    clear_bossfight_on_fight_end,
                    mark_run_over,
                    autosave_on_progress,
                )
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            )
            // Leaving play saves once, then refreshes the flag so `Continue` is up
            // to date by the time the title screen spawns.
            .add_systems(
                OnExit(GameState::Playing),
                (autosave_on_exit, refresh_continue_flag).chain(),
            )
            .add_systems(
                Update,
                (clear_save_on_new_game, handle_continue, handle_confirm).chain(),
            );
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
    pub points: ResMut<'w, RunPoints>,
    pub difficulty: ResMut<'w, Difficulty>,
    pub in_bossfight: ResMut<'w, InBossfight>,
    pub run_over: ResMut<'w, RunOver>,
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
            &self.points,
            self.in_bossfight.0,
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
        *self.points = data.points;
        *self.difficulty = data.difficulty;
        self.in_bossfight.0 = data.in_bossfight;
    }
}

/// Writes the run, logging rather than failing: a save error should never
/// interrupt play. An ended Permadeath run is never written — it is not
/// resumable — so this is a no-op while [`RunOver`] holds.
fn write_save(manager: &SaveManager, run: &RunState) {
    if run.run_over.0 {
        return;
    }
    match manager.save(&run.capture()) {
        Ok(()) => info!("saved run (day {})", run.day.0),
        Err(error) => warn!("could not save run: {error}"),
    }
}

/// Closes the run when a Permadeath death ends it: sets [`RunOver`] so nothing
/// is written, and drops the existing save so `Continue` cannot resurrect it.
fn mark_run_over(
    manager: Res<SaveManager>,
    mut run: RunState,
    mut died: MessageReader<PlayerDied>,
    mut flags: ResMut<MenuFeatureFlags>,
) {
    if died.read().count() == 0 {
        return;
    }
    if *run.difficulty != Difficulty::Permadeath || run.run_over.0 {
        return;
    }
    run.run_over.0 = true;
    if let Err(error) = manager.delete() {
        warn!("could not drop the ended run's save: {error}");
    }
    flags.load_game = false;
    info!("Permadeath run ended; its save has been cleared");
}

/// The authoritative close of a Permadeath run, run when the run-over screen is
/// entered. It catches the case the death-message path can miss (the message may
/// not have been seen while still `Playing`).
fn close_ended_run(
    manager: Res<SaveManager>,
    victory_mode: Res<VictoryMode>,
    mut run_over: ResMut<RunOver>,
    mut flags: ResMut<MenuFeatureFlags>,
) {
    if *victory_mode != VictoryMode::PermadeathDefeat || run_over.0 {
        return;
    }
    run_over.0 = true;
    if let Err(error) = manager.delete() {
        warn!("could not drop the ended run's save: {error}");
    }
    flags.load_game = false;
    info!("Permadeath run ended; its save has been cleared");
}

/// Marks the save as written mid-bossfight, and persists it immediately in
/// Permadeath so a forced quit is caught on the next load.
fn mark_bossfight(mut run: RunState, manager: Res<SaveManager>) {
    run.in_bossfight.0 = true;
    if *run.difficulty == Difficulty::Permadeath {
        write_save(&manager, &run);
    }
}

/// Clears the breadcrumb when the player is back on the farm.
fn clear_bossfight_on_farm(mut run: RunState, manager: Res<SaveManager>) {
    let was_fighting = run.in_bossfight.0;
    run.in_bossfight.0 = false;
    if was_fighting && *run.difficulty == Difficulty::Permadeath {
        write_save(&manager, &run);
    }
}

/// Clears the breadcrumb the moment a fight resolves, before the progress save
/// captures it, so a win or loss is never written as still-in-progress.
fn clear_bossfight_on_fight_end(
    mut run: RunState,
    mut defeated: MessageReader<BossDefeated>,
    mut died: MessageReader<PlayerDied>,
) {
    if defeated.read().count() + died.read().count() > 0 {
        run.in_bossfight.0 = false;
    }
}

/// Saves whenever the run made measurable progress.
fn autosave_on_progress(
    manager: Res<SaveManager>,
    run: RunState,
    mut day_advanced: MessageReader<DayAdvanced>,
    mut bosses: MessageReader<BossDefeated>,
    mut crafted: MessageReader<GearCrafted>,
    mut died: MessageReader<PlayerDied>,
    mut harvested: MessageReader<CropHarvested>,
) {
    let progressed = day_advanced.read().count()
        + bosses.read().count()
        + crafted.read().count()
        + died.read().count()
        + harvested.read().count()
        > 0;
    if progressed {
        write_save(&manager, &run);
    }
}

/// The catch-all save for leaving play, which includes returning to the menu.
fn autosave_on_exit(manager: Res<SaveManager>, run: RunState) {
    write_save(&manager, &run);
}

/// Reveals or hides the title screen's `Continue` slot to match the save slot,
/// and mirrors the saved run's points tally onto the title screen.
fn refresh_continue_flag(
    manager: Res<SaveManager>,
    mut flags: ResMut<MenuFeatureFlags>,
    mut points: ResMut<RunPoints>,
) {
    flags.load_game = manager.has_save();
    *points = manager.load().map(|data| data.points).unwrap_or_default();
}

/// Starting a new game drops the previous run's save, so `Continue` cannot
/// resurrect it before the new run has saved anything of its own.
fn clear_save_on_new_game(
    manager: Res<SaveManager>,
    mut requests: MessageReader<NewGameRequested>,
    mut run_over: ResMut<RunOver>,
    mut flags: ResMut<MenuFeatureFlags>,
) {
    if requests.read().count() == 0 {
        return;
    }
    if let Err(error) = manager.delete() {
        warn!("could not clear old save: {error}");
    }
    run_over.0 = false;
    flags.load_game = false;
}

/// The session resources a load touches, bundled so `handle_continue` stays
/// within the argument limit.
#[derive(SystemParam)]
struct LoadSession<'w> {
    resume: ResMut<'w, ResumeRun>,
    day_cycle: ResMut<'w, DayCycle>,
    dialog: ResMut<'w, ConfirmDialog>,
    flags: ResMut<'w, MenuFeatureFlags>,
}

/// Loads the saved run and begins play when the title screen asks to continue.
///
/// A tainted Permadeath save does not begin play: the run is invalidated (stepped
/// down to Apocalypse), the corrected save is written, and the player is shown a
/// notice to acknowledge first.
fn handle_continue(
    manager: Res<SaveManager>,
    mut run: RunState,
    mut requests: MessageReader<ContinueRequested>,
    mut fade: ResMut<SceneFade>,
    mut session: LoadSession,
) {
    if requests.read().count() == 0 {
        return;
    }
    let data = match manager.load() {
        Ok(data) => data,
        Err(error) => {
            // An unreadable save (or one from an older schema) must not leave a
            // `Continue` button that can never work.
            warn!("could not load run: {error}");
            let _ = manager.delete();
            session.flags.load_game = false;
            return;
        }
    };

    let tainted_permadeath = data.in_bossfight && data.difficulty == Difficulty::Permadeath;
    run.apply(data);
    // A resumed run keeps its saved day: clear any pending day advance and tell
    // the first-day boot to skip its tick-over. The breadcrumb never carries
    // into live play.
    *session.day_cycle = DayCycle::default();
    run.in_bossfight.0 = false;
    run.run_over.0 = false;
    session.resume.0 = true;

    if tainted_permadeath {
        *run.difficulty = Difficulty::Apocalypse;
        // Persist the correction so the notice is shown exactly once.
        let _ = manager.save(&run.capture());
        info!("invalidated a Permadeath run quit mid-fight; stepped down to Apocalypse");
        session.dialog.open(
            ConfirmKind::TaintNotice,
            "Permadeath Run Invalidated",
            "You quit a Permadeath run while a boss fight was live. \
             The run has been stepped down to Apocalypse so it can continue.",
            vec!["Acknowledge".to_string()],
        );
    } else {
        info!("continuing saved run (day {})", run.day.0);
        fade.begin_game(GameState::Playing);
    }
}

/// Starts the loaded run once the player acknowledges the invalidation notice.
fn handle_confirm(mut resolved: MessageReader<ConfirmResolved>, mut fade: ResMut<SceneFade>) {
    for event in resolved.read() {
        if event.kind == ConfirmKind::TaintNotice {
            fade.begin_game(GameState::Playing);
        }
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
        .init_state::<DayPhase>()
        .init_resource::<DayCounter>()
        .init_resource::<PlayerGear>()
        .init_resource::<Inventory>()
        .init_resource::<FarmState>()
        .init_resource::<CropUnlocks>()
        .init_resource::<BossProgress>()
        .init_resource::<GoalsProgress>()
        .init_resource::<RunPoints>()
        .init_resource::<Difficulty>()
        .add_message::<DayAdvanced>()
        .add_message::<BossDefeated>()
        .add_message::<GearCrafted>()
        .add_message::<CropHarvested>()
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

    fn set_phase(app: &mut App, phase: DayPhase) {
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(phase);
        app.update();
    }

    fn flags(app: &App) -> MenuFeatureFlags {
        *app.world().resource::<MenuFeatureFlags>()
    }

    fn save_of(app: &App) -> SaveData {
        app.world().resource::<SaveManager>().load().unwrap()
    }

    fn seed(app: &App, data: &SaveData) {
        app.world().resource::<SaveManager>().save(data).unwrap();
    }

    #[test]
    fn boot_reflects_an_existing_save() {
        let mut app = setup();
        // No save yet.
        app.update();
        assert!(!flags(&app).load_game);

        // Move off boot, seed a save, then boot again.
        enter_menu(&mut app);
        seed(
            &app,
            &SaveData::capture(
                &DayCounter(3),
                &PlayerGear::default(),
                &Inventory::default(),
                &FarmState::default(),
                &CropUnlocks::new(),
                &BossProgress::default(),
                &GoalsProgress::default(),
                Difficulty::Normal,
                &RunPoints::default(),
                false,
            ),
        );

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

        assert_eq!(save_of(&app).day, 7);
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
        assert_eq!(save_of(&app).day, 4, "leaving play saved the run");
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
    fn continuing_restores_the_run_and_enters_playing() {
        let mut app = setup();
        let mut gear = PlayerGear::default();
        gear.own(GearPiece::new(GearSet::Master, GearSlot::Weapon));
        let mut inventory = Inventory::default();
        inventory.add_crop(CropType::CropA, 9);
        seed(
            &app,
            &SaveData {
                version: SAVE_VERSION,
                day: 11,
                gear,
                inventory,
                farm: FarmState::default(),
                unlocks: CropUnlocks::new(),
                bosses: BossProgress::default(),
                goals: GoalsProgress::default(),
                difficulty: Difficulty::Apocalypse,
                points: RunPoints::default(),
                in_bossfight: false,
            },
        );

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
        assert!(
            app.world().resource::<ResumeRun>().0,
            "a resumed run tells the first-day boot to skip its advance"
        );
        assert!(
            !app.world().resource::<DayCycle>().pending_advance,
            "no stale day advance is carried into the loaded run"
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
    fn a_permadeath_death_drops_the_save_and_hides_continue() {
        let mut app = setup();
        *app.world_mut().resource_mut::<Difficulty>() = Difficulty::Permadeath;
        enter_playing(&mut app);
        // Entering the fight wrote a save, so a `Continue` would be offered.
        set_phase(&mut app, DayPhase::BossFight);
        assert!(app.world().resource::<SaveManager>().has_save());

        app.world_mut().write_message(PlayerDied);
        app.update();
        app.update();

        assert!(
            !app.world().resource::<SaveManager>().has_save(),
            "an ended Permadeath run is not resumable"
        );
        assert!(!flags(&app).load_game, "Continue disappears");
        assert!(app.world().resource::<RunOver>().0);
    }

    #[test]
    fn a_permadeath_death_writes_no_save_on_the_way_out() {
        let mut app = setup();
        *app.world_mut().resource_mut::<Difficulty>() = Difficulty::Permadeath;
        enter_playing(&mut app);
        set_phase(&mut app, DayPhase::BossFight);

        app.world_mut().write_message(PlayerDied);
        app.update();

        // Leaving `Playing` (as death does, for Victory) must not re-write a save.
        enter_menu(&mut app);

        assert!(!app.world().resource::<SaveManager>().has_save());
    }

    fn permadeath_save(in_bossfight: bool) -> SaveData {
        SaveData::capture(
            &DayCounter(6),
            &PlayerGear::default(),
            &Inventory::default(),
            &FarmState::default(),
            &CropUnlocks::new(),
            &BossProgress::default(),
            &GoalsProgress::default(),
            Difficulty::Permadeath,
            &RunPoints::default(),
            in_bossfight,
        )
    }

    #[test]
    fn entering_victory_after_a_permadeath_death_drops_the_save() {
        let mut app = setup();
        *app.world_mut().resource_mut::<VictoryMode>() = VictoryMode::PermadeathDefeat;
        seed(&app, &permadeath_save(false));

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();

        assert!(
            !app.world().resource::<SaveManager>().has_save(),
            "the run-over screen leaves nothing to continue"
        );
        assert!(app.world().resource::<RunOver>().0);
        assert!(!flags(&app).load_game);
    }

    #[test]
    fn a_permadeath_victory_keeps_the_save() {
        let mut app = setup();
        *app.world_mut().resource_mut::<VictoryMode>() = VictoryMode::PermadeathVictory;
        seed(&app, &permadeath_save(false));

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();

        assert!(
            app.world().resource::<SaveManager>().has_save(),
            "a won run still continues"
        );
        assert!(!app.world().resource::<RunOver>().0);
    }

    #[test]
    fn entering_a_permadeath_fight_taints_the_save() {
        let mut app = setup();
        *app.world_mut().resource_mut::<Difficulty>() = Difficulty::Permadeath;
        enter_playing(&mut app);

        set_phase(&mut app, DayPhase::BossFight);
        assert!(
            save_of(&app).in_bossfight,
            "the fight is written into the save so a force-quit is caught"
        );
    }

    #[test]
    fn finishing_a_fight_clears_the_taint() {
        let mut app = setup();
        *app.world_mut().resource_mut::<Difficulty>() = Difficulty::Permadeath;
        enter_playing(&mut app);
        set_phase(&mut app, DayPhase::BossFight);
        assert!(save_of(&app).in_bossfight);

        app.world_mut()
            .write_message(BossDefeated(crate::components::boss::BossId::BossA));
        app.update();

        assert!(
            !save_of(&app).in_bossfight,
            "a resolved fight is no longer written as in-progress"
        );
    }

    #[test]
    fn loading_a_tainted_permadeath_save_invalidates_it() {
        let mut app = setup();
        seed(&app, &permadeath_save(true));

        app.world_mut().write_message(ContinueRequested);
        for _ in 0..2 {
            app.update();
        }

        assert_eq!(
            app.world().resource::<Difficulty>(),
            &Difficulty::Apocalypse,
            "the run steps down to Apocalypse"
        );
        assert!(
            app.world().resource::<ConfirmDialog>().open,
            "the player is told to acknowledge"
        );
        assert_eq!(
            app.world().resource::<ConfirmDialog>().kind,
            ConfirmKind::TaintNotice
        );
        assert_ne!(
            app.world().resource::<State<GameState>>().get(),
            &GameState::Playing,
            "play waits for the acknowledgement"
        );
        assert!(
            !save_of(&app).in_bossfight,
            "the correction is persisted so the notice shows once"
        );
        assert_eq!(save_of(&app).difficulty, Difficulty::Apocalypse);
    }

    #[test]
    fn acknowledging_the_notice_starts_play() {
        let mut app = setup();
        seed(&app, &permadeath_save(true));
        app.world_mut().write_message(ContinueRequested);
        for _ in 0..2 {
            app.update();
        }
        assert!(app.world().resource::<ConfirmDialog>().open);

        app.world_mut().resource_mut::<ConfirmDialog>().selected = 0;
        app.world_mut().write_message(ConfirmResolved {
            kind: ConfirmKind::TaintNotice,
            choice: 0,
        });
        for _ in 0..3 {
            app.update();
        }

        assert_eq!(
            app.world().resource::<State<GameState>>().get(),
            &GameState::Playing
        );
    }

    #[test]
    fn an_untainted_permadeath_save_loads_normally() {
        let mut app = setup();
        seed(&app, &permadeath_save(false));

        app.world_mut().write_message(ContinueRequested);
        for _ in 0..3 {
            app.update();
        }

        assert_eq!(
            app.world().resource::<Difficulty>(),
            &Difficulty::Permadeath
        );
        assert!(!app.world().resource::<ConfirmDialog>().open);
        assert_eq!(
            app.world().resource::<State<GameState>>().get(),
            &GameState::Playing
        );
    }

    #[test]
    fn a_tainted_save_on_another_difficulty_is_ignored() {
        let mut app = setup();
        let mut data = permadeath_save(true);
        data.difficulty = Difficulty::Normal;
        seed(&app, &data);

        app.world_mut().write_message(ContinueRequested);
        for _ in 0..3 {
            app.update();
        }

        assert_eq!(app.world().resource::<Difficulty>(), &Difficulty::Normal);
        assert!(!app.world().resource::<ConfirmDialog>().open);
        assert_eq!(
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
            &RunPoints::default(),
            false,
        );
        manager.save(&data).unwrap();
        assert_eq!(manager.load().unwrap(), data);
    }
}
