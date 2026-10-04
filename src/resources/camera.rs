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
    }
}
