//! The per-run points tally.
//!
//! Points are a post-game currency: they stay locked (and hidden) until the
//! player first beats the dual boss, alongside Infinite Mode. Once unlocked,
//! harvesting a crop and felling a boss both add to the tally, which is carried
//! in the save.

use crate::components::pot::CropType;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Points awarded for felling any ordinary boss, in any mode.
pub const BOSS_KILL_POINTS: u32 = 10;

/// Points awarded for the dual boss, which counts as both of its halves.
pub const DUAL_KILL_POINTS: u32 = 20;

/// Set the first time the dual boss falls; cleared once the post-game unlock
/// notice has actually been shown. Transient (not saved): the notice waits for
/// the player to confirm they want to keep playing before it appears.
#[derive(Resource, Reflect, Default, Debug, Clone, Copy)]
pub struct PendingUnlockNotice(pub bool);

/// The run's points. `unlocked` flips the first time the dual boss falls; while
/// it is false nothing accumulates and the HUD hides the tally.
#[derive(Resource, Reflect, Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
pub struct RunPoints {
    pub unlocked: bool,
    pub total: u32,
}

impl RunPoints {
    /// Adds to the tally. A no-op while the system is still locked, so early
    /// harvests and kills before the dual boss fall earn nothing.
    pub fn add(&mut self, amount: u32) {
        if self.unlocked {
            self.total = self.total.saturating_add(amount);
        }
    }

    /// Unlocks the points system, reporting true only on the first unlock so the
    /// caller can show the unlock notice exactly once.
    pub fn unlock(&mut self) -> bool {
        if self.unlocked {
            false
        } else {
            self.unlocked = true;
            true
        }
    }
}

/// Points a harvested crop of this type is worth.
pub fn crop_points(crop: CropType) -> u32 {
    crop.points()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_tally_is_locked_and_empty() {
        let points = RunPoints::default();
        assert!(!points.unlocked);
        assert_eq!(points.total, 0);
    }

    #[test]
    fn locked_points_do_not_accumulate() {
        let mut points = RunPoints::default();
        points.add(10);
        assert_eq!(points.total, 0);
    }

    #[test]
    fn unlocking_returns_true_only_once() {
        let mut points = RunPoints::default();
        assert!(points.unlock());
        assert!(!points.unlock());
        assert!(points.unlocked);
    }

    #[test]
    fn unlocked_points_accumulate() {
        let mut points = RunPoints::default();
        points.unlock();
        points.add(3);
        points.add(10);
        assert_eq!(points.total, 13);
    }

    #[test]
    fn crops_are_worth_more_as_they_unlock() {
        assert_eq!(crop_points(CropType::Starter), 1);
        assert_eq!(crop_points(CropType::CropA), 2);
        assert_eq!(crop_points(CropType::CropB), 3);
    }
}
