use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The run's difficulty.
///
/// Difficulty only scales the bosses: their health pool and the damage their
/// attacks deal. It is read when a boss spawns, so changing it (from the pause
/// overlay) takes effect from the *next* boss fight onward rather than
/// retuning a fight already in progress.
///
/// `Permadeath` additionally changes how a run ends: dying sends the player to
/// the run-over screen instead of the usual defeat result, and it can only be
/// chosen before a run begins (never mid-run).
#[derive(
    Resource, Reflect, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, Default,
)]
pub enum Difficulty {
    Easy,
    #[default]
    Normal,
    Hard,
    Apocalypse,
    Permadeath,
}

impl Difficulty {
    /// Every difficulty, easiest first. The order is the picker's row order.
    pub const ALL: [Difficulty; 5] = [
        Difficulty::Easy,
        Difficulty::Normal,
        Difficulty::Hard,
        Difficulty::Apocalypse,
        Difficulty::Permadeath,
    ];

    /// The picker's row index for this difficulty.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|d| *d == self).unwrap_or(1)
    }

    pub fn label(self) -> &'static str {
        match self {
            Difficulty::Easy => "Easy",
            Difficulty::Normal => "Normal",
            Difficulty::Hard => "Hard",
            Difficulty::Apocalypse => "Apocalypse",
            Difficulty::Permadeath => "Permadeath",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Difficulty::Easy => "A gentler run. Bosses are frailer and hit softer.",
            Difficulty::Normal => "The intended balance, as the fights were tuned.",
            Difficulty::Hard => "Bosses endure longer and land heavier blows.",
            Difficulty::Apocalypse => "A punishing run. The bosses are far tougher.",
            Difficulty::Permadeath => {
                "One life. Death ends the run unless you step down to Apocalypse. Cannot be chosen mid-run."
            }
        }
    }

    /// Boss health multiplier. Applied when a boss spawns.
    ///
    /// Permadeath is a *rule* (one life), not a stat escalation, so its bosses
    /// are exactly as tough as Apocalypse's.
    pub fn boss_health_multiplier(self) -> f32 {
        match self {
            Difficulty::Easy => 0.75,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.30,
            Difficulty::Apocalypse | Difficulty::Permadeath => 1.60,
        }
    }

    /// Boss attack-damage multiplier. Baked into each attack a boss throws.
    pub fn boss_damage_multiplier(self) -> f32 {
        match self {
            Difficulty::Easy => 0.70,
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.25,
            Difficulty::Apocalypse | Difficulty::Permadeath => 1.55,
        }
    }

    /// A one-line effect summary for the picker row.
    pub fn summary(self) -> String {
        format!(
            "Boss HP x{:.2}   Damage x{:.2}",
            self.boss_health_multiplier(),
            self.boss_damage_multiplier()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_is_the_default() {
        assert_eq!(Difficulty::default(), Difficulty::Normal);
    }

    #[test]
    fn every_difficulty_has_a_distinct_label_and_description() {
        let mut labels: Vec<&str> = Difficulty::ALL.iter().map(|d| d.label()).collect();
        assert!(labels.iter().all(|label| !label.is_empty()));
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), Difficulty::ALL.len(), "duplicate label");

        for difficulty in Difficulty::ALL {
            assert!(!difficulty.description().is_empty());
        }
    }

    #[test]
    fn index_round_trips_through_all() {
        for (index, difficulty) in Difficulty::ALL.iter().enumerate() {
            assert_eq!(difficulty.index(), index);
            assert_eq!(Difficulty::ALL[difficulty.index()], *difficulty);
        }
    }

    #[test]
    fn multipliers_scale_up_with_difficulty() {
        let health: Vec<f32> = Difficulty::ALL
            .iter()
            .map(|d| d.boss_health_multiplier())
            .collect();
        let damage: Vec<f32> = Difficulty::ALL
            .iter()
            .map(|d| d.boss_damage_multiplier())
            .collect();
        for pair in health.windows(2) {
            assert!(pair[1] >= pair[0], "health must not drop with difficulty");
        }
        for pair in damage.windows(2) {
            assert!(pair[1] >= pair[0], "damage must not drop with difficulty");
        }
        assert_eq!(Difficulty::Normal.boss_health_multiplier(), 1.0);
        assert_eq!(Difficulty::Normal.boss_damage_multiplier(), 1.0);
    }

    #[test]
    fn permadeath_uses_apocalypse_boss_stats() {
        assert_eq!(
            Difficulty::Permadeath.boss_health_multiplier(),
            Difficulty::Apocalypse.boss_health_multiplier()
        );
        assert_eq!(
            Difficulty::Permadeath.boss_damage_multiplier(),
            Difficulty::Apocalypse.boss_damage_multiplier()
        );
    }

    #[test]
    fn summary_mentions_the_multipliers() {
        let summary = Difficulty::Hard.summary();
        assert!(summary.contains("HP"));
        assert!(summary.contains("Damage"));
    }
}
