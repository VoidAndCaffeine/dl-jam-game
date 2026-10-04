use bevy::prelude::*;

use crate::plugins::{FarmPlugin, InteractionPlugin};
use crate::resources::camera::CameraFollowConfig;
use crate::states::{DayPhase, GameState};
use crate::systems::camera_follow::camera_follow;
use crate::systems::collision::collision_detection;
use crate::systems::collision_response::collision_response;
use crate::systems::movement_input::movement_input;
use crate::systems::movement_physics::movement_physics;
use crate::systems::spawn_player::{despawn_player, spawn_player};
use crate::systems::transition::transition_to_playing;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .init_state::<DayPhase>()
            .init_resource::<CameraFollowConfig>()
            .add_plugins(InteractionPlugin)
            .add_plugins(FarmPlugin)
            .add_systems(OnEnter(GameState::LoadingAssets), transition_to_playing)
            .add_systems(OnEnter(GameState::Playing), spawn_player)
            .add_systems(OnExit(GameState::Playing), despawn_player)
            .add_systems(FixedUpdate, movement_input)
            .add_systems(FixedUpdate, movement_physics)
            .add_systems(FixedUpdate, collision_detection)
            .add_systems(FixedUpdate, camera_follow)
            .add_observer(collision_response);
    }
}
