use bevy::prelude::*;

/// The entity the player has locked onto, if any.
///
/// Lock-on is manual: it starts empty, the player cycles candidates with Z/X or
/// the mouse wheel, and it clears itself when the target dies or the fight ends.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct LockOn {
    pub target: Option<Entity>,
}

impl LockOn {
    pub fn is_locked_on(&self, entity: Entity) -> bool {
        self.target == Some(entity)
    }

    pub fn clear(&mut self) {
        self.target = None;
    }

    pub fn set(&mut self, entity: Entity) {
        self.target = Some(entity);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_on_starts_empty_and_can_be_set_and_cleared() {
        let mut lock = LockOn::default();
        assert!(lock.target.is_none());

        let mut world = World::new();
        let entity = world.spawn_empty().id();
        lock.set(entity);
        assert!(lock.is_locked_on(entity));
        assert_eq!(lock.target, Some(entity));

        lock.clear();
        assert!(lock.target.is_none());
    }
}
