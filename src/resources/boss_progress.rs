use crate::components::boss::BossId;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// What the run knows about the bosses so far.
#[derive(Resource, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct BossProgress {
    pub boss_a: bool,
    pub boss_b: bool,
    pub dual_boss_unlocked: bool,
    pub dual_boss_beaten: bool,
}

impl BossProgress {
    pub fn is_beaten(&self, id: BossId) -> bool {
        match id {
            BossId::BossA => self.boss_a,
            BossId::BossB => self.boss_b,
            BossId::Dual => self.dual_boss_beaten,
        }
    }

    /// The dual boss is only selectable once both single bosses are down.
    pub fn is_unlocked(&self, id: BossId) -> bool {
        match id {
            BossId::BossA | BossId::BossB => true,
            BossId::Dual => self.dual_boss_unlocked,
        }
    }

    /// Records a win and refreshes the dual boss unlock.
    pub fn record(&mut self, id: BossId) {
        match id {
            BossId::BossA => self.boss_a = true,
            BossId::BossB => self.boss_b = true,
            BossId::Dual => self.dual_boss_beaten = true,
        }
        if self.boss_a && self.boss_b {
            self.dual_boss_unlocked = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_single_bosses_start_available_and_dual_locked() {
        let progress = BossProgress::default();
        assert!(progress.is_unlocked(BossId::BossA));
        assert!(progress.is_unlocked(BossId::BossB));
        assert!(!progress.is_unlocked(BossId::Dual));
        assert!(!progress.is_beaten(BossId::Dual));
    }

    #[test]
    fn beating_both_single_bosses_unlocks_dual() {
        let mut progress = BossProgress::default();
        progress.record(BossId::BossA);
        assert!(!progress.dual_boss_unlocked);
        progress.record(BossId::BossB);
        assert!(progress.dual_boss_unlocked);
        assert!(progress.is_beaten(BossId::BossA));
        assert!(progress.is_beaten(BossId::BossB));
    }

    #[test]
    fn recording_dual_marks_the_run_beaten() {
        let mut progress = BossProgress::default();
        progress.record(BossId::Dual);
        assert!(progress.dual_boss_beaten);
    }
}
