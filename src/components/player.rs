use bevy::prelude::*;

pub const PLAYER_SIZE: f32 = 32.0;
// Interaction range = 2x pot size (pot size = 40.0)
pub const INTERACTION_RANGE: f32 = 80.0;

#[derive(Component, Reflect, Default, Clone)]
pub struct Player;

#[derive(Component, Reflect, Default, Clone)]
pub struct Health {
    pub current: f32,
    pub max: f32,
    /// Flat damage reduction applied before every hit.
    pub armor_reduction: f32,
    /// Seconds of invulnerability left after the last hit.
    pub iframe_remaining: f32,
}

impl Health {
    pub fn new(max: f32, armor_reduction: f32) -> Self {
        Self {
            current: max,
            max,
            armor_reduction,
            iframe_remaining: 0.0,
        }
    }

    /// Health as a 0.0..=1.0 fraction of the maximum.
    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 {
            0.0
        } else {
            (self.current / self.max).clamp(0.0, 1.0)
        }
    }

    pub fn is_invulnerable(&self) -> bool {
        self.iframe_remaining > 0.0
    }
}

#[derive(Component, Reflect, Clone)]
pub struct Movement {
    pub speed: f32,
    pub velocity: Vec2,
    pub input_direction: Vec2,
}

impl Default for Movement {
    fn default() -> Self {
        Self {
            speed: 200.0,
            velocity: Vec2::ZERO,
            input_direction: Vec2::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_default_values() {
        let m = Movement::default();
        assert_eq!(m.speed, 200.0);
        assert_eq!(m.velocity, Vec2::ZERO);
        assert_eq!(m.input_direction, Vec2::ZERO);
    }

    #[test]
    fn health_default_values() {
        let h = Health::default();
        assert_eq!(h.current, 0.0);
        assert_eq!(h.max, 0.0);
        assert_eq!(h.armor_reduction, 0.0);
        assert!(!h.is_invulnerable());
    }

    #[test]
    fn new_health_starts_full_and_fraction_tracks_damage() {
        let mut health = Health::new(100.0, 5.0);
        assert_eq!(health.current, 100.0);
        assert_eq!(health.max, 100.0);
        assert_eq!(health.armor_reduction, 5.0);
        assert_eq!(health.fraction(), 1.0);

        health.current = 25.0;
        assert_eq!(health.fraction(), 0.25);
    }

    #[test]
    fn a_zero_max_health_has_no_fraction() {
        assert_eq!(Health::default().fraction(), 0.0);
    }

    #[test]
    fn player_component_exists() {
        let _player = Player;
        // Just verify it compiles and can be constructed
    }
}
