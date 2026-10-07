use bevy::prelude::*;

/// Marks an entity the player's lock-on can select: bosses, dual halves and
/// mercury decoys.
#[derive(Component, Reflect, Debug, Default, Clone, Copy)]
pub struct Targetable;

/// A mercury decoy. It has a single point of health and bursts for splash
/// damage to the player when it is struck.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct Decoy {
    /// Splash radius when the decoy pops.
    pub splash_radius: f32,
    /// Splash damage dealt to the player.
    pub splash_damage: f32,
}

impl Decoy {
    pub fn new(splash_radius: f32, splash_damage: f32) -> Self {
        Self {
            splash_radius,
            splash_damage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_decoy_remembers_its_splash() {
        let decoy = Decoy::new(30.0, 12.0);
        assert_eq!(decoy.splash_radius, 30.0);
        assert_eq!(decoy.splash_damage, 12.0);
    }
}
