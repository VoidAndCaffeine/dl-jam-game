use bevy::prelude::*;

#[derive(Component, Reflect, Default, Clone)]
pub struct Player;

#[derive(Component, Reflect, Default)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

#[derive(Component, Reflect, Clone)]
pub struct Movement {
    pub speed: f32,
    pub velocity: Vec2,
    pub input_direction: Vec2,
}

impl Default for Movement {
    fn default() -> Self {
        Self {
            speed: 200.0,
            velocity: Vec2::ZERO,
            input_direction: Vec2::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movement_default_values() {
        let m = Movement::default();
        assert_eq!(m.speed, 200.0);
        assert_eq!(m.velocity, Vec2::ZERO);
        assert_eq!(m.input_direction, Vec2::ZERO);
    }

    #[test]
    fn health_default_values() {
        let h = Health::default();
        assert_eq!(h.current, 0.0);
        assert_eq!(h.max, 0.0);
    }

    #[test]
    fn player_component_exists() {
        let _player = Player;
        // Just verify it compiles and can be constructed
    }
}
