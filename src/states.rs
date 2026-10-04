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
}

#[cfg(test)]
mod tests {
    use super::*;

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
