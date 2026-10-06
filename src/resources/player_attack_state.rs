use bevy::prelude::*;

/// Cooldowns, rooting and facing for the player's swings.
///
/// Kept out of the player entity so the movement system can read it cheaply and
/// so it resets cleanly between runs.
#[derive(Resource, Reflect, Debug, Clone, Copy)]
pub struct PlayerAttackState {
    /// Seconds until the next light swing is allowed.
    pub light_cooldown: f32,
    /// Seconds until the next heavy swing is allowed.
    pub heavy_cooldown: f32,
    /// Seconds the player stays rooted in the current swing.
    pub attack_lock: f32,
    /// The direction the next swing will travel.
    pub facing: Vec2,
}

impl Default for PlayerAttackState {
    fn default() -> Self {
        Self {
            light_cooldown: 0.0,
            heavy_cooldown: 0.0,
            attack_lock: 0.0,
            facing: Vec2::X,
        }
    }
}

impl PlayerAttackState {
    /// Advances every timer by `dt`.
    pub fn tick(&mut self, dt: f32) {
        self.light_cooldown = (self.light_cooldown - dt).max(0.0);
        self.heavy_cooldown = (self.heavy_cooldown - dt).max(0.0);
        self.attack_lock = (self.attack_lock - dt).max(0.0);
    }

    pub fn is_rooted(&self) -> bool {
        self.attack_lock > 0.0
    }

    pub fn is_ready(&self, attack: crate::components::attack::AttackType) -> bool {
        match attack {
            crate::components::attack::AttackType::Light => self.light_cooldown <= 0.0,
            crate::components::attack::AttackType::Heavy => self.heavy_cooldown <= 0.0,
        }
    }

    /// Starts a swing: roots the player and starts the matching cooldown.
    pub fn begin(&mut self, attack: crate::components::attack::AttackType) {
        use crate::components::attack::AttackType;
        self.attack_lock = crate::constants::ATTACK_DURATION;
        match attack {
            AttackType::Light => self.light_cooldown = crate::constants::LIGHT_COOLDOWN,
            AttackType::Heavy => self.heavy_cooldown = crate::constants::HEAVY_COOLDOWN,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::attack::AttackType;

    #[test]
    fn default_faces_right_and_is_ready() {
        let state = PlayerAttackState::default();
        assert_eq!(state.facing, Vec2::X);
        assert!(state.is_ready(AttackType::Light));
        assert!(state.is_ready(AttackType::Heavy));
        assert!(!state.is_rooted());
    }

    #[test]
    fn beginning_a_light_swing_roots_and_cools_down_only_light() {
        let mut state = PlayerAttackState::default();
        state.begin(AttackType::Light);

        assert!(state.is_rooted());
        assert!(!state.is_ready(AttackType::Light));
        assert!(state.is_ready(AttackType::Heavy));
    }

    #[test]
    fn timers_run_down_over_time() {
        let mut state = PlayerAttackState::default();
        state.begin(AttackType::Heavy);
        state.tick(crate::constants::ATTACK_DURATION + 0.01);
        assert!(!state.is_rooted());

        state.tick(crate::constants::HEAVY_COOLDOWN);
        assert!(state.is_ready(AttackType::Heavy));
    }

    #[test]
    fn timers_never_go_negative() {
        let mut state = PlayerAttackState::default();
        state.tick(100.0);
        assert_eq!(state.light_cooldown, 0.0);
        assert_eq!(state.heavy_cooldown, 0.0);
        assert_eq!(state.attack_lock, 0.0);
    }
}
