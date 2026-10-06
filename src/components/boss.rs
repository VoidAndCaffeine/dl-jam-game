use crate::components::gear::MaterialType;
use crate::constants::{
    BOSS_A_HEALTH, BOSS_B_HEALTH, BOSS_PHASE2_MULTIPLIER, BOSS_WANDER_ANGULAR_SPEED,
    BOSS_WANDER_RADIUS, DUAL_BOSS_HEALTH, PHASE_STUN_DURATION, PHASE_THRESHOLD,
};
use crate::levels::LevelId;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::ops::RangeInclusive;

pub const BOSS_SIZE: f32 = 64.0;

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

    pub fn max_health(self) -> f32 {
        match self {
            BossId::BossA => BOSS_A_HEALTH,
            BossId::BossB => BOSS_B_HEALTH,
            BossId::Dual => DUAL_BOSS_HEALTH,
        }
    }

    /// The materials a defeat can drop, each with its inclusive roll range.
    /// Every boss yields its own two materials.
    pub fn material_drops(self) -> &'static [(MaterialType, RangeInclusive<u32>)] {
        match self {
            BossId::BossA => &[(MaterialType::BossA1, 0..=2), (MaterialType::BossA2, 1..=3)],
            BossId::BossB => &[(MaterialType::BossB1, 0..=2), (MaterialType::BossB2, 1..=3)],
            BossId::Dual => &[],
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
    /// 1 until the boss drops through [`PHASE_THRESHOLD`], then 2.
    pub phase: u8,
    /// Seconds left of the phase-change stun.
    pub stun_remaining: f32,
    /// Angle around the spawn point used by the placeholder wander.
    pub wander_angle: f32,
    /// Where the boss was spawned; the placeholder wander orbits this point.
    pub home: [f32; 2],
}

impl Boss {
    pub fn new(id: BossId) -> Self {
        Self::new_at(id, Vec2::ZERO)
    }

    pub fn new_at(id: BossId, home: Vec2) -> Self {
        let health = id.max_health();
        Self {
            id,
            health,
            max_health: health,
            phase: 1,
            stun_remaining: 0.0,
            wander_angle: 0.0,
            home: [home.x, home.y],
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

    pub fn health_fraction(&self) -> f32 {
        if self.max_health <= 0.0 {
            0.0
        } else {
            (self.health / self.max_health).clamp(0.0, 1.0)
        }
    }

    /// Movement speed multiplier: faster once the boss enrages.
    pub fn speed_multiplier(&self) -> f32 {
        if self.phase >= 2 {
            BOSS_PHASE2_MULTIPLIER
        } else {
            1.0
        }
    }

    /// Advances the placeholder brain by `dt` and returns the point the boss
    /// should walk towards, or `None` while stunned by a phase change.
    ///
    /// This is deliberately one self-contained step: real attack patterns can
    /// replace it without touching movement, rendering or damage code.
    pub fn wander_step(&mut self, dt: f32) -> Option<Vec2> {
        if self.phase == 1 && self.health_fraction() <= PHASE_THRESHOLD {
            self.phase = 2;
            self.stun_remaining = PHASE_STUN_DURATION;
        }
        if self.stun_remaining > 0.0 {
            self.stun_remaining = (self.stun_remaining - dt).max(0.0);
            return None;
        }

        self.wander_angle += BOSS_WANDER_ANGULAR_SPEED * self.speed_multiplier() * dt;
        let home = Vec2::new(self.home[0], self.home[1]);
        Some(
            home + Vec2::new(self.wander_angle.cos(), self.wander_angle.sin()) * BOSS_WANDER_RADIUS,
        )
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
    fn a_fresh_boss_starts_in_phase_one_at_full_health() {
        let boss = Boss::new(BossId::BossA);
        assert!(!boss.is_dead());
        assert_eq!(boss.health, BOSS_A_HEALTH);
        assert_eq!(boss.max_health, BOSS_A_HEALTH);
        assert_eq!(boss.phase, 1);
        assert_eq!(boss.health_fraction(), 1.0);
    }

    #[test]
    fn each_boss_has_its_own_health_pool() {
        assert_eq!(Boss::new(BossId::BossA).max_health, BOSS_A_HEALTH);
        assert_eq!(Boss::new(BossId::BossB).max_health, BOSS_B_HEALTH);
        assert_eq!(Boss::new(BossId::Dual).max_health, DUAL_BOSS_HEALTH);
    }

    #[test]
    fn a_boss_dies_when_damage_empties_its_pool() {
        let mut boss = Boss::new(BossId::BossA);
        assert!(!boss.damage(BOSS_A_HEALTH * 0.5));
        assert!(!boss.is_dead());
        assert!(boss.damage(BOSS_A_HEALTH * 0.5));
        assert!(boss.is_dead());
    }

    #[test]
    fn dropping_to_half_health_enrages_and_stuns_the_boss() {
        let mut boss = Boss::new(BossId::BossA);
        boss.health = boss.max_health * 0.6;
        assert!(boss.wander_step(0.1).is_some(), "still phase 1");

        boss.health = boss.max_health * 0.5;
        assert!(boss.wander_step(0.1).is_none(), "stunned on the transition");
        assert_eq!(boss.phase, 2);
        assert!(boss.stun_remaining > 0.0);
        assert_eq!(boss.speed_multiplier(), BOSS_PHASE2_MULTIPLIER);
    }

    #[test]
    fn the_stun_ends_after_its_duration() {
        let mut boss = Boss::new(BossId::BossB);
        boss.health = 0.0;
        assert!(boss.wander_step(PHASE_STUN_DURATION + 1.0).is_none());
        assert!(boss.wander_step(0.1).is_some());
    }

    #[test]
    fn wandering_orbits_the_spawn_point() {
        let home = Vec2::new(100.0, 50.0);
        let mut boss = Boss::new_at(BossId::BossA, home);
        let target = boss.wander_step(1.0).unwrap();
        assert!((target - home).length() - BOSS_WANDER_RADIUS < 0.001);
    }

    #[test]
    fn boss_default_id_is_boss_a() {
        assert_eq!(BossId::default(), BossId::BossA);
    }

    #[test]
    fn bosses_drop_their_own_materials() {
        assert_eq!(
            BossId::BossA.material_drops(),
            &[(MaterialType::BossA1, 0..=2), (MaterialType::BossA2, 1..=3),]
        );
        assert_eq!(
            BossId::BossB.material_drops(),
            &[(MaterialType::BossB1, 0..=2), (MaterialType::BossB2, 1..=3),]
        );
        assert!(BossId::Dual.material_drops().is_empty());
    }
}
