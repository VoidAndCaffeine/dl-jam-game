use bevy::prelude::*;

#[derive(Resource, Reflect)]
pub struct CameraFollowConfig {
    pub deadzone_size: Vec2,
    pub lerp_factor: f32,
}

impl CameraFollowConfig {
    pub const DEFAULT: Self = Self {
        deadzone_size: Vec2::new(80.0, 50.0),
        lerp_factor: 0.15,
    };
}

impl Default for CameraFollowConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}