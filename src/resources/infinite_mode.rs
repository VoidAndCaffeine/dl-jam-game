//! State for the post-game Infinite Mode fight.
//!
//! Infinite Mode is entered from boss select once the dual boss has fallen. It
//! runs in the dual arena, but instead of the two halves sharing one health
//! pool, the Excavator and the Quicksilver fight independently with their own
//! health bars. When one falls it is worth its kill points and, after a short
//! timer, it climbs back up — tougher each time. There is no victory: the fight
//! only ends when the player dies, which returns them to the farm as usual.

use crate::components::boss::DualRole;
use bevy::prelude::*;

/// Seconds a felled Infinite Mode boss stays down before it respawns.
pub const RESPAWN_SECONDS: f32 = 15.0;

/// Extra health multiplier a boss gains for each time it has already fallen, so
/// the endurance run ramps up as it goes.
pub const RESPAWN_HEALTH_STEP: f32 = 0.15;

/// Live Infinite Mode state. Transient: it never enters the save, and is cleared
/// the moment the fight ends.
#[derive(Resource, Reflect, Default, Debug, Clone, Copy, PartialEq)]
pub struct InfiniteMode {
    pub active: bool,
    /// True once the opening pair has been fielded, so the fight is never set up
    /// twice (the respawn timers own the bosses after that).
    pub spawned: bool,
    /// Seconds until the Excavator climbs back up, while it is down.
    pub excavator_timer: Option<f32>,
    /// Seconds until the Quicksilver climbs back up, while it is down.
    pub quicksilver_timer: Option<f32>,
    pub excavator_deaths: u32,
    pub quicksilver_deaths: u32,
}

impl InfiniteMode {
    /// Starts a fresh endurance run.
    pub fn begin(&mut self) {
        *self = Self {
            active: true,
            ..Self::default()
        };
    }

    /// Clears the run when the fight ends.
    pub fn end(&mut self) {
        *self = Self::default();
    }

    pub fn timer(&self, role: DualRole) -> Option<f32> {
        match role {
            DualRole::Excavator => self.excavator_timer,
            DualRole::Quicksilver => self.quicksilver_timer,
        }
    }

    pub fn set_timer(&mut self, role: DualRole, timer: Option<f32>) {
        match role {
            DualRole::Excavator => self.excavator_timer = timer,
            DualRole::Quicksilver => self.quicksilver_timer = timer,
        }
    }

    /// Records one of this role's falls, which toughens its next appearance.
    pub fn record_death(&mut self, role: DualRole) {
        match role {
            DualRole::Excavator => self.excavator_deaths += 1,
            DualRole::Quicksilver => self.quicksilver_deaths += 1,
        }
    }

    pub fn deaths(&self, role: DualRole) -> u32 {
        match role {
            DualRole::Excavator => self.excavator_deaths,
            DualRole::Quicksilver => self.quicksilver_deaths,
        }
    }

    /// The health scale for this role's next appearance: 1.0 the first time,
    /// climbing by [`RESPAWN_HEALTH_STEP`] for each fall it has already taken.
    pub fn health_multiplier(&self, role: DualRole) -> f32 {
        1.0 + RESPAWN_HEALTH_STEP * self.deaths(role) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_mode_is_inactive() {
        let mode = InfiniteMode::default();
        assert!(!mode.active);
        assert_eq!(mode.timer(DualRole::Excavator), None);
        assert_eq!(mode.timer(DualRole::Quicksilver), None);
    }

    #[test]
    fn beginning_starts_clean_and_active() {
        let mut mode = InfiniteMode::default();
        mode.record_death(DualRole::Excavator);
        mode.begin();
        assert!(mode.active);
        assert_eq!(mode.deaths(DualRole::Excavator), 0);
        assert_eq!(mode.timer(DualRole::Quicksilver), None);
    }

    #[test]
    fn ending_clears_everything() {
        let mut mode = InfiniteMode::default();
        mode.begin();
        mode.record_death(DualRole::Quicksilver);
        mode.set_timer(DualRole::Quicksilver, Some(3.0));
        mode.end();
        assert_eq!(mode, InfiniteMode::default());
    }

    #[test]
    fn timers_are_per_role() {
        let mut mode = InfiniteMode::default();
        mode.set_timer(DualRole::Excavator, Some(11.0));
        assert_eq!(mode.timer(DualRole::Excavator), Some(11.0));
        assert_eq!(mode.timer(DualRole::Quicksilver), None);
        mode.set_timer(DualRole::Excavator, None);
        assert_eq!(mode.timer(DualRole::Excavator), None);
    }

    #[test]
    fn each_fall_toughens_the_next_appearance() {
        let mut mode = InfiniteMode::default();
        assert_eq!(mode.health_multiplier(DualRole::Excavator), 1.0);
        mode.record_death(DualRole::Excavator);
        assert!((mode.health_multiplier(DualRole::Excavator) - 1.15).abs() < 1e-6);
        mode.record_death(DualRole::Excavator);
        assert!((mode.health_multiplier(DualRole::Excavator) - 1.30).abs() < 1e-6);
        assert_eq!(mode.health_multiplier(DualRole::Quicksilver), 1.0);
    }
}
