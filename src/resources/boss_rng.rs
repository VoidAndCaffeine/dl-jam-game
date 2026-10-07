use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};

/// Rolls boss pattern choices and scatter positions. Separate from `DropRng` so
/// loot rolls stay reproducible when tests pin a seed.
#[derive(Resource)]
pub struct BossRng(pub SmallRng);

impl Default for BossRng {
    fn default() -> Self {
        Self::seeded(0)
    }
}

impl BossRng {
    pub fn seeded(seed: u64) -> Self {
        Self(SmallRng::seed_from_u64(seed))
    }

    /// A random value in `0..n`, or 0 when `n` is 0.
    pub fn index(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { self.0.random_range(0..n) }
    }

    /// A random float in `0.0..1.0`.
    pub fn unit(&mut self) -> f32 {
        self.0.random_range(0.0..1.0)
    }

    /// A random float in `-1.0..1.0`.
    pub fn signed_unit(&mut self) -> f32 {
        self.0.random_range(-1.0..1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_repeats_the_sequence() {
        let mut a = BossRng::seeded(9);
        let mut b = BossRng::seeded(9);
        let first: Vec<usize> = (0..12).map(|_| a.index(6)).collect();
        let second: Vec<usize> = (0..12).map(|_| b.index(6)).collect();
        assert_eq!(first, second);
        assert!(first.iter().all(|value| *value < 6));
    }
}
