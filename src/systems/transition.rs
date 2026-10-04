use bevy::prelude::*;

use crate::states::GameState;

pub fn transition_to_playing(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::Playing);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;
    use bevy::state::app::StatesPlugin;

    #[test]
    fn loading_assets_transitions_to_playing() {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.add_systems(OnEnter(GameState::LoadingAssets), transition_to_playing);
        
        // Start in LoadingAssets
        app.world_mut().resource_mut::<NextState<GameState>>().set(GameState::LoadingAssets);
        app.update();
        
        assert_eq!(app.world().resource::<State<GameState>>().get(), &GameState::Playing);
    }
}
