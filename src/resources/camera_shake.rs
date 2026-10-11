use bevy::prelude::*;

/// How far, in world units, the camera is pushed at full trauma.
pub const MAX_SHAKE_OFFSET: f32 = 13.0;
/// Trauma bled off per second. At this rate a full-trauma knock is over in
/// roughly half a second.
pub const TRAUMA_DECAY: f32 = 1.8;

/// Screen-shake state.
///
/// Events add `trauma` (0..=1); a driver decays it every fixed step and bakes an
/// offset into the camera transform. `applied` remembers the offset currently
/// baked in so the follow spring can be given back the camera's true position
/// before it integrates.
#[derive(Resource, Reflect, Debug, Default)]
pub struct CameraShake {
    trauma: f32,
    applied: Vec2,
}

impl CameraShake {
    /// Adds trauma, saturating at full.
    pub fn add(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).clamp(0.0, 1.0);
    }

    pub fn trauma(&self) -> f32 {
        self.trauma
    }

    /// The offset currently baked into the camera transform.
    pub fn applied(&self) -> Vec2 {
        self.applied
    }

    /// Shake strength in 0..=1. Squaring the trauma keeps small knocks subtle
    /// and makes a real hit punch.
    pub fn intensity(&self) -> f32 {
        self.trauma * self.trauma
    }

    pub fn set_trauma(&mut self, trauma: f32) {
        self.trauma = trauma.clamp(0.0, 1.0);
    }

    pub fn set_applied(&mut self, offset: Vec2) {
        self.applied = offset;
    }

    /// Drops the shake outright, for a hard scene cut.
    pub fn reset(&mut self) {
        self.trauma = 0.0;
        self.applied = Vec2::ZERO;
    }
}

/// An offset for `intensity`, from a noise sample whose components run -1..=1.
pub fn shake_offset(intensity: f32, noise: Vec2) -> Vec2 {
    noise.clamp(Vec2::splat(-1.0), Vec2::splat(1.0)) * MAX_SHAKE_OFFSET * intensity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trauma_saturates_at_full_and_intensity_squares_it() {
        let mut shake = CameraShake::default();
        shake.add(0.3);
        assert_eq!(shake.trauma(), 0.3);
        assert!((shake.intensity() - 0.09).abs() < 1e-6, "0.3 squared");

        shake.add(5.0);
        assert_eq!(shake.trauma(), 1.0, "cannot exceed full trauma");
        assert_eq!(shake.intensity(), 1.0);
    }

    #[test]
    fn an_offset_scales_with_intensity_and_stays_in_bounds() {
        let noise = Vec2::splat(1.0);
        assert_eq!(shake_offset(0.0, noise), Vec2::ZERO);
        assert_eq!(shake_offset(1.0, noise), Vec2::splat(MAX_SHAKE_OFFSET));
        assert_eq!(
            shake_offset(0.5, noise),
            Vec2::splat(MAX_SHAKE_OFFSET * 0.5)
        );
    }

    #[test]
    fn noisy_samples_are_clamped_to_the_unit_box() {
        let wild = shake_offset(1.0, Vec2::new(4.0, -3.0));
        assert!(wild.x <= MAX_SHAKE_OFFSET && wild.y >= -MAX_SHAKE_OFFSET);
    }

    #[test]
    fn reset_clears_both_trauma_and_the_applied_offset() {
        let mut shake = CameraShake::default();
        shake.add(1.0);
        shake.set_applied(Vec2::new(3.0, -2.0));
        shake.reset();
        assert_eq!(shake.trauma(), 0.0);
        assert_eq!(shake.applied(), Vec2::ZERO);
    }
}
