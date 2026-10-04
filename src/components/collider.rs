use bevy::prelude::*;

#[derive(Component, Reflect, Default)]
pub struct Collider {
    pub size: Vec2,
    pub is_solid: bool,
}

#[derive(Event, Debug, Clone)]
pub struct CollisionEvent {
    pub entity_a: Entity,
    pub entity_b: Entity,
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
    fn collision_event_creation() {
        // Just verify CollisionEvent can be created with entities
        let mut app = App::new();
        let e1 = app.world_mut().spawn_empty().id();
        let e2 = app.world_mut().spawn_empty().id();
        let event = CollisionEvent {
            entity_a: e1,
            entity_b: e2,
        };
        assert_eq!(event.entity_a, e1);
        assert_eq!(event.entity_b, e2);
    }
}
