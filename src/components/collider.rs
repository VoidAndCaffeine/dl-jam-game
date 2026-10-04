use bevy::prelude::*;

#[derive(Component, Reflect, Default)]
pub struct Collider {
    pub size: Vec2,
    pub is_solid: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collider_default_values() {
        let c = Collider::default();
        assert_eq!(c.size, Vec2::ZERO);
        assert!(!c.is_solid);
    }

    #[test]
    fn a_collider_can_be_built_for_a_world_box() {
        let collider = Collider {
            size: Vec2::splat(32.0),
            is_solid: true,
        };
        assert!(collider.is_solid);
        assert_eq!(collider.size * 0.5, Vec2::splat(16.0));
    }
}
