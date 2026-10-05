use crate::levels::LevelId;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub const BOSS_SIZE: f32 = 64.0;
/// Placeholder bosses die from a single interaction for now.
pub const BOSS_HEALTH: f32 = 1.0;

#[derive(
    Component, Reflect, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Default, Debug,
)]
pub enum BossId {
    #[default]
    BossA,
    BossB,
    Dual,
}

impl BossId {
    pub const ALL: [BossId; 3] = [BossId::BossA, BossId::BossB, BossId::Dual];

    pub fn label(self) -> &'static str {
        match self {
            BossId::BossA => "Boss A",
            BossId::BossB => "Boss B",
            BossId::Dual => "Dual Boss",
        }
    }

    /// The arena the boss is fought in.
    pub fn arena(self) -> LevelId {
        match self {
            BossId::BossA => LevelId::ArenaA,
            BossId::BossB => LevelId::ArenaB,
            BossId::Dual => LevelId::ArenaDual,
        }
    }

    pub fn color(self) -> Color {
        match self {
            BossId::BossA => Color::srgb(0.85, 0.25, 0.25),
            BossId::BossB => Color::srgb(0.25, 0.5, 0.85),
            BossId::Dual => Color::srgb(0.60, 0.25, 0.75),
        }
    }
}

/// The boss to spawn for a given room, if the room is an arena.
pub fn boss_for_level(level: LevelId) -> Option<BossId> {
    BossId::ALL.into_iter().find(|id| id.arena() == level)
}

#[derive(Component, Reflect, Serialize, Deserialize, Clone, Copy, Debug)]
pub struct Boss {
    pub id: BossId,
    pub health: f32,
    pub max_health: f32,
}

impl Boss {
    pub fn new(id: BossId) -> Self {
        Self {
            id,
            health: BOSS_HEALTH,
            max_health: BOSS_HEALTH,
        }
    }

    /// Applies damage and reports whether the boss died from it.
    pub fn damage(&mut self, amount: f32) -> bool {
        self.health -= amount;
        self.is_dead()
    }

    pub fn is_dead(&self) -> bool {
        self.health <= 0.0
    }
}

/// Tags a live boss so room changes and fight exits can clean it up.
#[derive(Component, Reflect, Debug, Default)]
pub struct BossSpawnMarker;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_boss_has_its_own_arena_and_label() {
        let mut arenas: Vec<LevelId> = Vec::new();
        for id in BossId::ALL {
            assert!(!id.label().is_empty());
            assert!(!arenas.contains(&id.arena()), "duplicate arena");
            arenas.push(id.arena());
        }
        assert_eq!(arenas.len(), 3);
    }

    #[test]
    fn boss_for_level_matches_an_arena_and_rejects_others() {
        assert_eq!(boss_for_level(LevelId::ArenaA), Some(BossId::BossA));
        assert_eq!(boss_for_level(LevelId::ArenaB), Some(BossId::BossB));
        assert_eq!(boss_for_level(LevelId::ArenaDual), Some(BossId::Dual));
        assert_eq!(boss_for_level(LevelId::Farm), None);
    }

    #[test]
    fn a_fresh_boss_is_alive_and_dies_in_one_hit() {
        let mut boss = Boss::new(BossId::BossA);
        assert!(!boss.is_dead());
        assert_eq!(boss.health, BOSS_HEALTH);
        assert!(!boss.damage(BOSS_HEALTH * 0.5));
        assert!(!boss.is_dead());
        assert!(boss.damage(BOSS_HEALTH * 0.5));
        assert!(boss.is_dead());
    }

    #[test]
    fn boss_default_id_is_boss_a() {
        assert_eq!(BossId::default(), BossId::BossA);
    }
}
