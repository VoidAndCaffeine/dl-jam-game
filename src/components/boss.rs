use crate::components::gear::MaterialType;
use crate::constants::{
    BOSS_A_HEALTH, BOSS_A_SIZE, BOSS_B_HEALTH, BOSS_B_SIZE, BOSS_PHASE2_MULTIPLIER,
    BOSS_WANDER_ANGULAR_SPEED, BOSS_WANDER_RADIUS, DUAL_BOSS_HEALTH, PHASE_STUN_DURATION,
    PHASE_THRESHOLD,
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
            BossId::BossA => "The Excavator",
            BossId::BossB => "Mercurial",
            BossId::Dual => "The Excavator & Mercurial",
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

    /// The sprite size of this boss.
    pub fn size(self) -> f32 {
        match self {
            BossId::BossA => BOSS_A_SIZE,
            BossId::BossB => BOSS_B_SIZE,
            BossId::Dual => BOSS_A_SIZE,
        }
    }

    /// The three attack patterns this boss draws from.
    pub fn patterns(self) -> &'static [PatternType] {
        PatternType::for_boss(self)
    }

    pub fn is_dual(self) -> bool {
        self == BossId::Dual
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

/// The attack kit a boss cycles through.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PatternType {
    // Boss A - The Excavator
    TailingsSurge,
    ExcavatorSlam,
    DebrisRain,
    // Boss B - The Quicksilver
    MirrorStep,
    QuicksilverWave,
    MadnessSpray,
    // Dual only
    Amalgamation,
}

impl PatternType {
    pub fn label(self) -> &'static str {
        match self {
            PatternType::TailingsSurge => "Tailings Surge",
            PatternType::ExcavatorSlam => "Excavator Slam",
            PatternType::DebrisRain => "Debris Rain",
            PatternType::MirrorStep => "Mirror Step",
            PatternType::QuicksilverWave => "Quicksilver Wave",
            PatternType::MadnessSpray => "Madness Spray",
            PatternType::Amalgamation => "Amalgamation",
        }
    }

    /// The three patterns an ordinary fight draws from.
    pub fn for_boss(id: BossId) -> &'static [PatternType] {
        match id {
            BossId::BossA => &[
                PatternType::TailingsSurge,
                PatternType::ExcavatorSlam,
                PatternType::DebrisRain,
            ],
            BossId::BossB => &[
                PatternType::MirrorStep,
                PatternType::QuicksilverWave,
                PatternType::MadnessSpray,
            ],
            BossId::Dual => &[
                PatternType::TailingsSurge,
                PatternType::ExcavatorSlam,
                PatternType::DebrisRain,
            ],
        }
    }

    /// Whether this pattern belongs to the industrial Excavator half.
    pub fn is_excavator(self) -> bool {
        matches!(
            self,
            PatternType::TailingsSurge | PatternType::ExcavatorSlam | PatternType::DebrisRain
        )
    }
}

/// Which half of the dual boss an entity is. Dual halves share one health pool
/// but keep their own patterns.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DualRole {
    Excavator,
    Quicksilver,
}

impl DualRole {
    pub fn id(self) -> BossId {
        match self {
            DualRole::Excavator => BossId::BossA,
            DualRole::Quicksilver => BossId::BossB,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DualRole::Excavator => "The Excavator",
            DualRole::Quicksilver => "The Quicksilver",
        }
    }

    /// The other half, used to alternate attack turns.
    pub fn other(self) -> DualRole {
        match self {
            DualRole::Excavator => DualRole::Quicksilver,
            DualRole::Quicksilver => DualRole::Excavator,
        }
    }
}

/// The attack kit state machine for one live boss.
#[derive(Component, Reflect, Debug, Clone)]
pub struct BossBrain {
    /// Pattern currently winding up or resolving, if any.
    pub current: Option<PatternType>,
    /// Seconds left before the next pattern may be chosen.
    pub cooldown: f32,
    /// The two most recently used patterns, so runs stay varied.
    pub history: [Option<PatternType>; 2],
    /// Seconds left of a one-shot coordination lock (dual Amalgamation).
    pub locked: f32,
}

impl Default for BossBrain {
    fn default() -> Self {
        Self {
            current: None,
            cooldown: 0.0,
            history: [None, None],
            locked: 0.0,
        }
    }
}

impl BossBrain {
    /// A brain that waits out the fight's opening buffer before its first pick.
    pub fn with_opening_grace() -> Self {
        Self {
            cooldown: crate::constants::BOSS_OPENING_GRACE,
            ..Self::default()
        }
    }

    /// Whether `pattern` was one of the last two picks.
    pub fn recently_used(&self, pattern: PatternType) -> bool {
        self.history.contains(&Some(pattern))
    }

    /// Records a pattern as the newest pick, dropping the oldest.
    pub fn remember(&mut self, pattern: PatternType) {
        self.history[0] = self.history[1];
        self.history[1] = Some(pattern);
    }
}

/// What a spawned hostile entity does. One component keeps the tick logic in a
/// single match instead of a dozen tiny systems.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackKind {
    /// A lingering caustic smear left by a charge.
    SurgeTrail,
    /// A pool of acid that damages and slows.
    AcidPool,
    /// A ground slam that winds up, then bursts.
    Slam,
    /// A falling ore chunk; armed once it lands.
    Debris,
    /// A slick mercury puddle that damages and hurries movement.
    MercuryPool,
    /// A targetable mercury decoy that bursts when struck.
    Decoy,
    /// A travelling wall of quicksilver.
    Wave,
    /// A droplet from the madness spray.
    Spray,
    /// A homing droplet from the phase-2 spray.
    Wisp,
    /// The dual bosses' shared channel, exploding for heavy damage.
    Amalgam,
}

impl AttackKind {
    /// Hazards that linger and hurt anything standing in them.
    pub fn is_ground_hazard(self) -> bool {
        matches!(
            self,
            AttackKind::SurgeTrail
                | AttackKind::AcidPool
                | AttackKind::MercuryPool
                | AttackKind::Amalgam
        )
    }

    /// Whether the telegraph sprite is drawn while the attack winds up.
    ///
    /// Ground attacks show their landing marker; thrown attacks stay hidden so
    /// the boss's own windup clip is the only tell.
    pub fn shows_windup(self) -> bool {
        matches!(
            self,
            AttackKind::Slam | AttackKind::Debris | AttackKind::Amalgam
        )
    }

    /// Thrown attacks re-aim at the player the moment their windup ends.
    pub fn aims_at_player(self) -> bool {
        matches!(
            self,
            AttackKind::Wave | AttackKind::Spray | AttackKind::Wisp
        )
    }
}

/// One runtime-spawned boss attack or hazard.
#[derive(Component, Reflect, Debug, Clone)]
pub struct BossAttack {
    pub kind: AttackKind,
    /// The boss that created it, for friendly fire and cleanup.
    pub owner: Option<Entity>,
    /// Whether the attack was created in the boss's enraged phase.
    pub phase: u8,
    /// Where it started, and where it is now (movers update this).
    pub position: Vec2,
    /// Where a telegraph is aiming.
    pub target: Vec2,
    /// Move direction for projectiles and charges.
    pub direction: Vec2,
    /// Current travel speed, if it moves.
    pub speed: f32,
    /// Effect radius / half-size, depending on kind.
    pub radius: f32,
    /// Damage per application.
    pub damage: f32,
    /// Seconds left before it despawns.
    pub remaining: f32,
    /// Total lifetime, for fading effects.
    pub total: f32,
    /// Seconds until it may damage the player again.
    pub hit_cooldown: f32,
    /// Scratch timer for behaviours that spawn things over time (wave pools).
    pub aux_timer: f32,
    /// Seconds left before a telegraph winds up and the attack activates.
    pub windup: f32,
    /// Offset from the boss's aim used to fan out thrown attacks (sprays).
    pub aim_offset: f32,
    /// False while a telegraph is still winding up.
    pub armed: bool,
    /// Set the first frame the attack activates, so one-shot effects fire once.
    pub impacted: bool,
}

impl BossAttack {
    pub fn new(kind: AttackKind, position: Vec2) -> Self {
        Self {
            kind,
            owner: None,
            phase: 1,
            position,
            target: position,
            direction: Vec2::X,
            speed: 0.0,
            radius: 24.0,
            damage: 0.0,
            remaining: 1.0,
            total: 1.0,
            hit_cooldown: 0.0,
            aux_timer: 0.0,
            windup: 0.0,
            aim_offset: 0.0,
            armed: true,
            impacted: false,
        }
    }

    pub fn owned_by(mut self, owner: Entity) -> Self {
        self.owner = Some(owner);
        self
    }

    pub fn with_phase(mut self, phase: u8) -> Self {
        self.phase = phase;
        self
    }

    pub fn with_lifetime(mut self, seconds: f32) -> Self {
        self.remaining = seconds;
        self.total = seconds;
        self
    }

    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    pub fn with_damage(mut self, damage: f32) -> Self {
        self.damage = damage;
        self
    }

    /// Delays activation by `seconds`; the attack stays inert until then.
    pub fn with_windup(mut self, seconds: f32) -> Self {
        self.windup = seconds;
        self.armed = false;
        self
    }

    /// The fan offset from the boss's aim, used by sprays.
    pub fn with_aim_offset(mut self, offset: f32) -> Self {
        self.aim_offset = offset;
        self
    }
}

/// A mirror-step teleport waiting out its windup before the boss blinks.
///
/// The boss stays put (and visible) until `windup` elapses, then jumps to
/// `destination` and leaves its decoys behind.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct PendingBlink {
    pub windup: f32,
    pub destination: Vec2,
    pub decoys: u32,
    pub phase: u8,
}

/// Tags every attack entity a boss fight spawned, so the arena can be swept
/// clean when the fight ends.
#[derive(Component, Reflect, Debug, Default)]
pub struct BossEncounterEntity;

/// Marks a boss that has run out of health and is playing its death clip.
///
/// The boss is kept alive (and immune to further damage) until `remaining`
/// runs out, at which point the defeat is announced and it despawns.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct Dying {
    pub remaining: f32,
}

impl Dying {
    pub fn new(remaining: f32) -> Self {
        Self { remaining }
    }
}

/// Attached to a boss while it is performing a Tailings Surge charge.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct SurgeCharger {
    pub direction: Vec2,
    pub speed: f32,
    /// Charges still to run (phase 2 runs two).
    pub charges_left: u32,
    /// Seconds left of the pre-charge windup.
    pub windup: f32,
    /// Seconds left of the current charge.
    pub active: f32,
    /// Counts down to the next trail smear.
    pub trail_timer: f32,
    /// Radians per second the charge path bends (phase 2 curves).
    pub curve: f32,
}

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
    fn a_fresh_brain_waits_out_the_opening_grace() {
        assert_eq!(BossBrain::default().cooldown, 0.0);
        assert_eq!(
            BossBrain::with_opening_grace().cooldown,
            crate::constants::BOSS_OPENING_GRACE
        );
        assert!(BossBrain::with_opening_grace().current.is_none());
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
