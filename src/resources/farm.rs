use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Resource, Reflect, Serialize, Deserialize, Default, Debug)]
pub struct DayCounter(pub u32);

impl DayCounter {
    pub fn advance(&mut self) {
        self.0 += 1;
    }
}

#[derive(Resource, Reflect, Serialize, Deserialize, Default, Debug)]
pub struct CropUnlocks {
    pub starter: bool,
    pub crop_a: bool,
    pub crop_b: bool,
}

impl CropUnlocks {
    pub fn new() -> Self {
        Self {
            starter: true,
            crop_a: false,
            crop_b: false,
        }
    }

    pub fn is_unlocked(&self, crop_type: crate::components::pot::CropType) -> bool {
        match crop_type {
            crate::components::pot::CropType::Starter => self.starter,
            crate::components::pot::CropType::CropA => self.crop_a,
            crate::components::pot::CropType::CropB => self.crop_b,
        }
    }

    pub fn unlock_crop_a(&mut self) {
        self.crop_a = true;
    }

    pub fn unlock_crop_b(&mut self) {
        self.crop_b = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_counter_default() {
        let counter = DayCounter::default();
        assert_eq!(counter.0, 0);
    }

    #[test]
    fn day_counter_advance() {
        let mut counter = DayCounter::default();
        counter.advance();
        assert_eq!(counter.0, 1);
        counter.advance();
        assert_eq!(counter.0, 2);
    }

    #[test]
    fn crop_unlocks_default() {
        let unlocks = CropUnlocks::new();
        assert!(unlocks.starter);
        assert!(!unlocks.crop_a);
        assert!(!unlocks.crop_b);
    }

    #[test]
    fn crop_unlocks_is_unlocked() {
        let unlocks = CropUnlocks::new();
        assert!(unlocks.is_unlocked(crate::components::pot::CropType::Starter));
        assert!(!unlocks.is_unlocked(crate::components::pot::CropType::CropA));
        assert!(!unlocks.is_unlocked(crate::components::pot::CropType::CropB));
    }

    #[test]
    fn crop_unlocks_unlock_crop_a() {
        let mut unlocks = CropUnlocks::new();
        unlocks.unlock_crop_a();
        assert!(unlocks.is_unlocked(crate::components::pot::CropType::CropA));
    }

    #[test]
    fn crop_unlocks_unlock_crop_b() {
        let mut unlocks = CropUnlocks::new();
        unlocks.unlock_crop_b();
        assert!(unlocks.is_unlocked(crate::components::pot::CropType::CropB));
    }
}