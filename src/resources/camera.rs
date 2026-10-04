use bevy::prelude::*;

#[derive(Resource, Reflect)]
pub struct CameraFollowConfig {
    pub half_life: f32,
    /// How much world the camera can see, used to keep it inside the room.
    pub visible_size: Vec2,
}

impl CameraFollowConfig {
    pub const DEFAULT: Self = Self {
        half_life: 0.1,
        visible_size: Vec2::new(1280.0, 720.0),
    };
}

impl Default for CameraFollowConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_follow_config_default() {
        let config = CameraFollowConfig::default();
        assert_eq!(config.half_life, 0.1);
    }

    #[test]
    fn camera_follow_config_const_default() {
        let config = CameraFollowConfig::DEFAULT;
        assert_eq!(config.half_life, 0.1);
        assert_eq!(config.visible_size, Vec2::new(1280.0, 720.0));
    }
}
