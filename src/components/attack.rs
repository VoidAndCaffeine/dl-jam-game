use crate::constants::{HEAVY_COOLDOWN, HEAVY_MULTIPLIER, LIGHT_COOLDOWN, LIGHT_MULTIPLIER};
use bevy::prelude::*;

/// Which swing the player threw.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AttackType {
    #[default]
    Light,
    Heavy,
}

impl AttackType {
    pub fn label(self) -> &'static str {
        match self {
            AttackType::Light => "Light",
            AttackType::Heavy => "Heavy",
        }
    }

    /// Multiplier applied to the weapon's base damage.
    pub fn multiplier(self) -> f32 {
        match self {
            AttackType::Light => LIGHT_MULTIPLIER,
            AttackType::Heavy => HEAVY_MULTIPLIER,
        }
    }

    pub fn cooldown(self) -> f32 {
        match self {
            AttackType::Light => LIGHT_COOLDOWN,
            AttackType::Heavy => HEAVY_COOLDOWN,
        }
    }
}

/// A live swing.
///
/// The graphic itself is drawn by the attack-effect shader; this component
/// carries the lifetime and the hitbox the debug overlay draws, and despawns
/// the swing once `remaining` runs out.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct AttackVisual {
    pub attack_type: AttackType,
    pub remaining: f32,
    pub total: f32,
    /// The swing's damage footprint (reach × width), for the debug overlay.
    pub hitbox: Vec2,
}

impl AttackVisual {
    pub fn new(attack_type: AttackType, hitbox: Vec2) -> Self {
        Self {
            attack_type,
            remaining: crate::constants::ATTACK_DURATION,
            total: crate::constants::ATTACK_DURATION,
            hitbox,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_is_the_default() {
        assert_eq!(AttackType::default(), AttackType::Light);
    }

    #[test]
    fn heavy_hits_harder_and_recovers_slower() {
        assert!(AttackType::Heavy.multiplier() > AttackType::Light.multiplier());
        assert!(AttackType::Heavy.cooldown() > AttackType::Light.cooldown());
    }

    #[test]
    fn attack_labels_are_distinct() {
        assert_ne!(AttackType::Light.label(), AttackType::Heavy.label(),);
    }

    #[test]
    fn a_fresh_visual_starts_at_its_full_duration() {
        let visual = AttackVisual::new(AttackType::Heavy, Vec2::new(70.0, 28.0));
        assert_eq!(visual.remaining, visual.total);
        assert_eq!(visual.attack_type, AttackType::Heavy);
        assert_eq!(visual.hitbox, Vec2::new(70.0, 28.0));
        assert!(visual.total > 0.0);
    }
}
