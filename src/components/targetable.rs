use bevy::prelude::*;

/// Marks an entity the player's lock-on can select: bosses, dual halves and
/// mercury decoys.
#[derive(Component, Reflect, Debug, Default, Clone, Copy)]
pub struct Targetable;

/// A mercury decoy. It has a single point of health and leaves a mercury
/// puddle behind when it is struck.
#[derive(Component, Reflect, Debug, Default, Clone, Copy)]
pub struct Decoy;
