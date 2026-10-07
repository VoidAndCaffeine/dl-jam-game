use crate::constants::{MERCURY_SLIP, SLOW_MULTIPLIER};
use bevy::prelude::*;

/// Transient effects a boss can inflict on the player.
///
/// Madness spray reverses the player's controls; acid slows them; mercury makes
/// the ground slick and speeds them up. All timers run down on their own so a
/// status always expires.
#[derive(Resource, Reflect, Debug, Clone, Copy, Default)]
pub struct PlayerStatus {
    /// Seconds left of reversed controls.
    pub reversed: f32,
    /// Seconds left of the slow.
    pub slowed: f32,
    /// Seconds left of mercury slip.
    pub slipping: f32,
}

impl PlayerStatus {
    pub fn tick(&mut self, dt: f32) {
        self.reversed = (self.reversed - dt).max(0.0);
        self.slowed = (self.slowed - dt).max(0.0);
        self.slipping = (self.slipping - dt).max(0.0);
    }

    pub fn is_reversed(&self) -> bool {
        self.reversed > 0.0
    }

    pub fn apply_reversal(&mut self, seconds: f32) {
        self.reversed = self.reversed.max(seconds);
    }

    pub fn apply_slow(&mut self, seconds: f32) {
        self.slowed = self.slowed.max(seconds);
    }

    pub fn apply_slip(&mut self, seconds: f32) {
        self.slipping = self.slipping.max(seconds);
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Movement multiplier from the strongest active status.
    pub fn speed_multiplier(&self) -> f32 {
        if self.slipping > 0.0 {
            MERCURY_SLIP
        } else if self.slowed > 0.0 {
            SLOW_MULTIPLIER
        } else {
            1.0
        }
    }

    /// Flips an input direction left/right when controls are reversed.
    pub fn apply_to(&self, direction: Vec2) -> Vec2 {
        if self.is_reversed() {
            Vec2::new(-direction.x, direction.y)
        } else {
            direction
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_expire() {
        let mut status = PlayerStatus::default();
        status.apply_reversal(2.0);
        status.apply_slow(1.5);
        status.apply_slip(1.0);
        assert!(status.is_reversed());
        status.tick(3.0);
        assert!(!status.is_reversed());
        assert_eq!(status.speed_multiplier(), 1.0);
    }

    #[test]
    fn reversal_flips_x_only() {
        let mut status = PlayerStatus::default();
        status.apply_reversal(1.0);
        assert_eq!(status.apply_to(Vec2::new(1.0, 2.0)), Vec2::new(-1.0, 2.0));
    }

    #[test]
    fn mercury_slip_outranks_slow() {
        let mut status = PlayerStatus::default();
        status.apply_slow(5.0);
        status.apply_slip(1.0);
        assert_eq!(status.speed_multiplier(), MERCURY_SLIP);
    }

    #[test]
    fn overlapping_reversals_keep_the_longest() {
        let mut status = PlayerStatus::default();
        status.apply_reversal(2.0);
        status.apply_reversal(1.0);
        assert_eq!(status.reversed, 2.0);
    }

    #[test]
    fn clear_resets_every_status() {
        let mut status = PlayerStatus::default();
        status.apply_reversal(1.0);
        status.apply_slow(1.0);
        status.clear();
        assert_eq!(status.reversed, 0.0);
        assert_eq!(status.slowed, 0.0);
    }
}
