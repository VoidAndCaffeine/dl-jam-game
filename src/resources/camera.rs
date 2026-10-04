use bevy::prelude::*;

#[derive(Resource, Reflect)]
pub struct CameraFollowConfig {
    pub half_life: f32,
}

impl CameraFollowConfig {
    pub const DEFAULT: Self = Self { half_life: 0.1 };
}

impl Default for CameraFollowConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}
