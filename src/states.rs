use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    LoadingAssets,
    Playing,
    Victory,
}

#[derive(SubStates, Debug, Clone, PartialEq, Eq, Hash, Default)]
#[source(GameState = GameState::Playing)]
pub enum DayPhase {
    #[default]
    Farming,
    BossSelect,
    BossFight,
    Result,
    /// Preloading the next scene's sheets. World interaction is frozen and the
    /// loading screen is up until the target has settled.
    Loading,
}

/// The two states most systems ask about, bundled so their signatures stay
/// readable.
///
/// `DayPhase` is a sub-state, so its resource is absent while `GameState` is
/// not `Playing`; the field is optional so systems that run during the boot
/// `LoadingAssets` state stay valid instead of panicking.
#[derive(SystemParam)]
pub struct Phase<'w> {
    pub game: Res<'w, State<GameState>>,
    day: Option<Res<'w, State<DayPhase>>>,
}

impl Phase<'_> {
    /// The current day phase, or `None` while not in `Playing`.
    pub fn day_phase(&self) -> Option<DayPhase> {
        self.day.as_ref().map(|state| state.get().clone())
    }

    pub fn is_playing(&self) -> bool {
        matches!(self.game.get(), GameState::Playing)
    }

    pub fn is_loading(&self) -> bool {
        self.is_playing() && self.day_phase() == Some(DayPhase::Loading)
    }

    pub fn is_farming(&self) -> bool {
        self.is_playing() && self.day_phase() == Some(DayPhase::Farming)
    }

    pub fn is_boss_select(&self) -> bool {
        self.is_playing() && self.day_phase() == Some(DayPhase::BossSelect)
    }

    pub fn is_boss_fight(&self) -> bool {
        self.is_playing() && self.day_phase() == Some(DayPhase::BossFight)
    }

    pub fn is_result(&self) -> bool {
        self.is_playing() && self.day_phase() == Some(DayPhase::Result)
    }

    /// True when a panel that pauses world interaction may react to input.
    /// Loading is not one of those: the world is frozen but no panel should
    /// respond.
    pub fn blocks_world(&self) -> bool {
        self.is_playing() && !self.is_loading()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    #[derive(Resource, Default)]
    struct Probe {
        playing: bool,
        farming: bool,
        blocks_world: bool,
    }

    fn record(phase: Phase, mut probe: ResMut<Probe>) {
        probe.playing = phase.is_playing();
        probe.farming = phase.is_farming();
        probe.blocks_world = phase.blocks_world();
    }

    fn setup() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_resource::<Probe>()
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_systems(Update, record);
        app.update();
        app
    }

    fn probe(app: &App) -> &Probe {
        app.world().resource::<Probe>()
    }

    fn set_game(app: &mut App, state: GameState) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(state);
        app.update();
    }

    fn set_phase(app: &mut App, phase: DayPhase) {
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(phase);
        app.update();
    }

    #[test]
    fn loading_assets_is_neither_playing_nor_farming() {
        let app = setup();
        assert!(!probe(&app).playing);
        assert!(!probe(&app).farming);
        assert!(!probe(&app).blocks_world);
    }

    #[test]
    fn playing_defaults_to_the_farming_phase() {
        let mut app = setup();
        set_game(&mut app, GameState::Playing);
        assert!(probe(&app).playing);
        assert!(probe(&app).farming);
    }

    #[test]
    fn a_boss_fight_stops_farming_but_keeps_panels_reachable() {
        let mut app = setup();
        set_game(&mut app, GameState::Playing);
        set_phase(&mut app, DayPhase::BossFight);

        assert!(probe(&app).playing);
        assert!(!probe(&app).farming);
        assert!(probe(&app).blocks_world);
    }

    #[test]
    fn boss_select_and_boss_fight_are_distinct_phases() {
        let mut app = setup();
        set_game(&mut app, GameState::Playing);

        set_phase(&mut app, DayPhase::BossSelect);
        assert!(probe(&app).playing);
        assert!(!probe(&app).farming);

        set_phase(&mut app, DayPhase::BossFight);
        assert!(!probe(&app).farming);
    }

    #[test]
    fn victory_ends_play_entirely() {
        let mut app = setup();
        set_game(&mut app, GameState::Playing);
        set_game(&mut app, GameState::Victory);

        assert!(!probe(&app).playing);
        assert!(!probe(&app).farming);
        assert!(!probe(&app).blocks_world);
    }

    #[test]
    fn game_state_default_is_loading_assets() {
        let state = GameState::default();
        assert_eq!(state, GameState::LoadingAssets);
    }

    #[test]
    fn day_phase_default_is_farming() {
        let phase = DayPhase::default();
        assert_eq!(phase, DayPhase::Farming);
    }
}
