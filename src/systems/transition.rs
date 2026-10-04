use bevy::prelude::*;

use crate::states::GameState;

pub fn transition_to_playing(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::Playing);
}
