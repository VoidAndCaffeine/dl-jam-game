use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(
    Component, Reflect, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default, Debug,
)]
pub enum PotState {
    #[default]
    Empty,
    Planted,
    Watered,
    Ready,
}

#[derive(
    Component, Reflect, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default, Debug,
)]
pub enum CropType {
    #[default]
    Starter,
    CropA,
    CropB,
}

impl CropType {
    pub fn growth_days(&self) -> u8 {
        match self {
            CropType::Starter => 3,
            CropType::CropA => 4,
            CropType::CropB => 5,
        }
    }
}

#[derive(Component, Reflect, Serialize, Deserialize, Default, Debug)]
pub struct Pot {
    pub index: usize,
    pub state: PotState,
    pub days_remaining: u8,
    pub watered_today: bool,
    pub crop_type: CropType,
}

impl Pot {
    pub fn new(index: usize) -> Self {
        Self {
            index,
            state: PotState::Empty,
            days_remaining: 0,
            watered_today: false,
            crop_type: CropType::Starter,
        }
    }

    pub fn plant(&mut self, crop_type: CropType) {
        self.state = PotState::Planted;
        self.crop_type = crop_type;
        self.days_remaining = crop_type.growth_days();
        self.watered_today = false;
    }

    pub fn water(&mut self) -> bool {
        if self.state == PotState::Planted && !self.watered_today {
            self.state = PotState::Watered;
            self.watered_today = true;
            true
        } else {
            false
        }
    }

    pub fn advance_day(&mut self) {
        if self.watered_today && self.days_remaining > 0 {
            self.days_remaining -= 1;
            if self.days_remaining == 0 {
                self.state = PotState::Ready;
            } else {
                self.state = PotState::Planted;
            }
        }
        self.watered_today = false;
    }

    pub fn harvest(&mut self) -> Option<CropType> {
        if self.state == PotState::Ready {
            let crop = self.crop_type;
            self.state = PotState::Empty;
            self.days_remaining = 0;
            self.watered_today = false;
            Some(crop)
        } else {
            None
        }
    }

    pub fn color(&self) -> Color {
        match self.state {
            PotState::Empty => Color::srgb(1.0, 0.2, 0.2),
            PotState::Planted => Color::srgb(0.2, 1.0, 0.2),
            PotState::Watered => Color::srgb(0.2, 0.4, 1.0),
            PotState::Ready => Color::srgb(0.8, 0.2, 0.8),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pot_default_is_empty() {
        let pot = Pot::new(0);
        assert_eq!(pot.state, PotState::Empty);
        assert_eq!(pot.days_remaining, 0);
        assert!(!pot.watered_today);
    }

    #[test]
    fn pot_plant_sets_correct_values() {
        let mut pot = Pot::new(0);
        pot.plant(CropType::Starter);
        assert_eq!(pot.state, PotState::Planted);
        assert_eq!(pot.days_remaining, 3);
        assert_eq!(pot.crop_type, CropType::Starter);
        assert!(!pot.watered_today);
    }

    #[test]
    fn pot_plant_crop_a() {
        let mut pot = Pot::new(0);
        pot.plant(CropType::CropA);
        assert_eq!(pot.days_remaining, 4);
    }

    #[test]
    fn pot_plant_crop_b() {
        let mut pot = Pot::new(0);
        pot.plant(CropType::CropB);
        assert_eq!(pot.days_remaining, 5);
    }

    #[test]
    fn pot_water_works_once_per_day() {
        let mut pot = Pot::new(0);
        pot.plant(CropType::Starter);
        assert!(pot.water());
        assert_eq!(pot.state, PotState::Watered);
        assert!(pot.watered_today);
        assert!(!pot.water());
    }

    #[test]
    fn pot_advance_day_decrements_timer() {
        let mut pot = Pot::new(0);
        pot.plant(CropType::Starter);
        pot.water();
        pot.advance_day();
        assert_eq!(pot.days_remaining, 2);
        assert_eq!(pot.state, PotState::Planted);
        assert!(!pot.watered_today);
    }

    #[test]
    fn pot_advance_day_ready_when_zero() {
        let mut pot = Pot::new(0);
        pot.plant(CropType::Starter);
        pot.water();
        pot.advance_day();
        pot.water();
        pot.advance_day();
        pot.water();
        pot.advance_day();
        assert_eq!(pot.days_remaining, 0);
        assert_eq!(pot.state, PotState::Ready);
    }

    #[test]
    fn pot_harvest_returns_crop_and_resets() {
        let mut pot = Pot::new(0);
        pot.plant(CropType::Starter);
        pot.water();
        pot.advance_day();
        pot.water();
        pot.advance_day();
        pot.water();
        pot.advance_day();
        let crop = pot.harvest();
        assert_eq!(crop, Some(CropType::Starter));
        assert_eq!(pot.state, PotState::Empty);
        assert_eq!(pot.days_remaining, 0);
    }

    #[test]
    fn pot_harvest_only_when_ready() {
        let mut pot = Pot::new(0);
        pot.plant(CropType::Starter);
        assert_eq!(pot.harvest(), None);
        pot.water();
        assert_eq!(pot.harvest(), None);
    }

    #[test]
    fn crop_type_growth_days() {
        assert_eq!(CropType::Starter.growth_days(), 3);
        assert_eq!(CropType::CropA.growth_days(), 4);
        assert_eq!(CropType::CropB.growth_days(), 5);
    }

    #[test]
    fn pot_colors_match_states() {
        let mut pot = Pot::new(0);
        assert_eq!(pot.color(), Color::srgb(1.0, 0.2, 0.2));
        pot.plant(CropType::Starter);
        assert_eq!(pot.color(), Color::srgb(0.2, 1.0, 0.2));
        pot.water();
        assert_eq!(pot.color(), Color::srgb(0.2, 0.4, 1.0));
        pot.advance_day();
        pot.water();
        pot.advance_day();
        pot.water();
        pot.advance_day();
        assert_eq!(pot.color(), Color::srgb(0.8, 0.2, 0.8));
    }
}
