use crate::components::boss::DualRole;
use crate::constants::DUAL_BOSS_HEALTH;
use bevy::prelude::*;

/// The single health pool both dual bosses draw from. Damage to either half
/// reduces this; at zero both fall together.
#[derive(Resource, Reflect, Debug, Clone, Copy)]
pub struct SharedBossHealth {
    pub current: f32,
    pub max: f32,
}

impl Default for SharedBossHealth {
    fn default() -> Self {
        Self::fresh()
    }
}

impl SharedBossHealth {
    pub fn fresh() -> Self {
        Self {
            current: DUAL_BOSS_HEALTH,
            max: DUAL_BOSS_HEALTH,
        }
    }

    pub fn fraction(&self) -> f32 {
        if self.max <= 0.0 {
            0.0
        } else {
            (self.current / self.max).clamp(0.0, 1.0)
        }
    }

    /// Applies damage and reports whether the shared pool is now empty.
    pub fn damage(&mut self, amount: f32) -> bool {
        self.current -= amount;
        self.current <= 0.0
    }

    pub fn reset(&mut self) {
        *self = Self::fresh();
    }
}

/// Tracks the dual fight's coordinated combo and Amalgamation cadence.
#[derive(Resource, Reflect, Debug, Clone, Copy)]
pub struct BossCoordinator {
    /// True while a dual encounter is running.
    pub active: bool,
    /// Seconds until the next coordinated combo.
    pub next_combo: f32,
    /// Seconds until the next Amalgamation.
    pub next_amalgamation: f32,
    /// True while the bosses are channelling an Amalgamation.
    pub channelling: bool,
    /// Seconds left of the current channel.
    pub channel_remaining: f32,
    /// Which half attacks on the current turn; they alternate.
    pub turn: DualRole,
}

impl Default for BossCoordinator {
    fn default() -> Self {
        Self {
            active: false,
            next_combo: crate::constants::DUAL_COMBO_INTERVAL,
            next_amalgamation: crate::constants::AMALGAMATION_INTERVAL_P1,
            channelling: false,
            channel_remaining: 0.0,
            turn: DualRole::Excavator,
        }
    }
}

impl BossCoordinator {
    pub fn begin(&mut self) {
        *self = Self {
            active: true,
            next_combo: crate::constants::BOSS_OPENING_GRACE,
            ..Default::default()
        };
    }

    /// Hands the next attack to the other half.
    pub fn flip_turn(&mut self) {
        self.turn = self.turn.other();
    }

    /// Schedules the next Amalgamation for the current health phase.
    pub fn schedule_amalgamation(&mut self, enraged: bool) {
        self.next_amalgamation = if enraged {
            crate::constants::AMALGAMATION_INTERVAL_P2
        } else {
            crate::constants::AMALGAMATION_INTERVAL_P1
        };
    }

    pub fn end(&mut self) {
        self.active = false;
        self.channelling = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_health_dies_when_emptied() {
        let mut health = SharedBossHealth::fresh();
        assert_eq!(health.current, DUAL_BOSS_HEALTH);
        assert!(!health.damage(DUAL_BOSS_HEALTH * 0.5));
        assert!(health.damage(DUAL_BOSS_HEALTH * 0.5));
        assert_eq!(health.fraction(), 0.0);
    }

    #[test]
    fn shared_health_reset_restores_the_pool() {
        let mut health = SharedBossHealth::fresh();
        health.damage(1000.0);
        health.reset();
        assert_eq!(health.current, DUAL_BOSS_HEALTH);
    }

    #[test]
    fn coordinator_toggles_with_the_fight() {
        let mut coord = BossCoordinator::default();
        assert!(!coord.active);
        coord.begin();
        assert!(coord.active);
        assert_eq!(coord.next_combo, crate::constants::BOSS_OPENING_GRACE);
        assert_eq!(
            coord.next_amalgamation,
            crate::constants::AMALGAMATION_INTERVAL_P1
        );
        coord.end();
        assert!(!coord.active);
    }
}
