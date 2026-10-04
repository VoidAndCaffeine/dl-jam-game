use bevy::prelude::*;

#[derive(Component, Reflect, Default)]
pub struct Player;

#[derive(Component, Reflect, Default)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

#[derive(Component, Reflect)]
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
