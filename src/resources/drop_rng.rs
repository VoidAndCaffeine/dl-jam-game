use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};
use std::ops::RangeInclusive;

/// Rolls boss material drops. Reseeded once per run on entering `Playing` so
/// every run differs; tests can pin a seed to make the rolls deterministic.
#[derive(Resource)]
pub struct DropRng(pub SmallRng);

impl Default for DropRng {
    /// A placeholder seed. `seed_drop_rng` overwrites it when play starts.
    fn default() -> Self {
        Self::seeded(0)
    }
}

impl DropRng {
    pub fn seeded(seed: u64) -> Self {
        Self(SmallRng::seed_from_u64(seed))
    }

    pub fn roll(&mut self, range: RangeInclusive<u32>) -> u32 {
        self.0.random_range(range)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_two_rolls_cover_every_value_in_range() {
        let mut rng = DropRng::seeded(1);
        let mut seen = [false; 4];
        for _ in 0..1000 {
            let roll = rng.roll(1..=3);
            assert!((1..=3).contains(&roll));
            seen[roll as usize] = true;
        }
        assert!(seen[1] && seen[2] && seen[3], "every value should appear");
    }

    #[test]
    fn material_one_can_roll_zero() {
        let mut rng = DropRng::seeded(1);
        let mut saw_zero = false;
        for _ in 0..1000 {
            assert!(rng.roll(0..=2) <= 2);
            saw_zero |= rng.roll(0..=2) == 0;
        }
        assert!(saw_zero, "material 1 should be able to drop nothing");
    }

    #[test]
    fn the_same_seed_repeats_the_same_sequence() {
        let mut first = DropRng::seeded(42);
        let mut second = DropRng::seeded(42);
        let a: Vec<u32> = (0..16).map(|_| first.roll(0..=2)).collect();
        let b: Vec<u32> = (0..16).map(|_| second.roll(0..=2)).collect();
        assert_eq!(a, b);
    }
}
