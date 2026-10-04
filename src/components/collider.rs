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
