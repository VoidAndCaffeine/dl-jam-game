use crate::resources::player_status::PlayerStatus;
use crate::states::{DayPhase, Phase};
use bevy::prelude::*;

/// Runs down the player's transient statuses, and wipes them when a fight ends.
pub fn tick_player_status(time: Res<Time>, mut status: ResMut<PlayerStatus>, phase: Phase) {
    if !phase.is_playing() {
        status.clear();
        return;
    }
    if matches!(phase.day.get(), DayPhase::BossFight) {
        status.tick(time.delta_secs());
    } else {
        status.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::states::GameState;
    use bevy::state::app::StatesPlugin;

    fn setup() -> App {
        let mut app = App::new();
        app.init_resource::<PlayerStatus>()
            .add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_systems(Update, tick_player_status);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossFight);
        app.update();
        app
    }

    #[test]
    fn a_reversal_runs_down_during_a_fight() {
        let mut app = setup();
        app.world_mut()
            .resource_mut::<PlayerStatus>()
            .apply_reversal(5.0);
        for _ in 0..4 {
            app.update();
        }
        let status = app.world().resource::<PlayerStatus>();
        assert!(status.reversed < 5.0, "the timer should have ticked");
    }

    #[test]
    fn statuses_clear_outside_a_fight() {
        let mut app = setup();
        app.world_mut()
            .resource_mut::<PlayerStatus>()
            .apply_reversal(5.0);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);
        app.update();
        assert!(!app.world().resource::<PlayerStatus>().is_reversed());
    }
}
