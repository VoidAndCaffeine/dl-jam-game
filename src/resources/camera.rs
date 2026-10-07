use bevy::prelude::*;

/// Apparent scale of the world. This is an orthographic projection `scale`,
/// which is inverse to how large objects appear, so `0.5` is a 2x zoom-in.
pub const CAMERA_ZOOM: f32 = 0.5;

#[derive(Resource, Reflect)]
pub struct CameraFollowConfig {
    pub half_life: f32,
    /// How much world the camera can see, used to keep it inside the room.
    /// This tracks [`CAMERA_ZOOM`]: at a 2x zoom the view is half of the
    /// 1280x720 base, so the camera clamps to the correct edges.
    pub visible_size: Vec2,
}

impl CameraFollowConfig {
    pub const DEFAULT: Self = Self {
        half_life: 0.1,
        visible_size: Vec2::new(640.0, 360.0),
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
        assert_eq!(config.visible_size, Vec2::new(640.0, 360.0));
    }

    #[test]
    fn a_two_times_zoom_halves_the_visible_size() {
        assert_eq!(CAMERA_ZOOM, 0.5);
        assert_eq!(
            CameraFollowConfig::DEFAULT.visible_size,
            Vec2::new(1280.0, 720.0) * CAMERA_ZOOM
        );
    }
}
