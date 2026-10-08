use crate::components::boss::AttackKind;
use crate::constants::{
    ACID_POOL_RADIUS, AMALGAMATION_RADIUS, DEBRIS_IMPACT_RADIUS, DEBRIS_SHADOW_RADIUS, DECOY_SIZE,
    MERCURY_POOL_RADIUS, SLAM_RADIUS, SLAM_RADIUS_P2, SURGE_TRAIL_LIFE, SURGE_TRAIL_WIDTH,
    SURGE_TRAIL_WIDTH_P2, WAVE_LENGTH, WAVE_WIDTH, WISP_SPRITE_SIZE,
};
use bevy::prelude::*;

/// One animated effect sheet. Effects are drawn flat and rotated at runtime, so
/// a single sheet serves every direction.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum EffectKind {
    TailingsSurge,
    ExcavatorSlam,
    AcidPool,
    /// The soft ground disc that marks where a falling attack will land.
    DebrisShadow,
    DebrisImpact,
    QuicksilverWave,
    MercuryPool,
    MadnessSpray,
    Wisp,
    AmalgamationBlast,
    /// Reuses the Mercuril boss idle art; no dedicated sheet was drawn.
    Decoy,
}

impl EffectKind {
    pub const ALL: [EffectKind; 11] = [
        EffectKind::TailingsSurge,
        EffectKind::ExcavatorSlam,
        EffectKind::AcidPool,
        EffectKind::DebrisShadow,
        EffectKind::DebrisImpact,
        EffectKind::QuicksilverWave,
        EffectKind::MercuryPool,
        EffectKind::MadnessSpray,
        EffectKind::Wisp,
        EffectKind::AmalgamationBlast,
        EffectKind::Decoy,
    ];

    /// The folder inside `assets/sprite_packs/Effects`. Decoy borrows a boss
    /// pack instead, so it has no folder here.
    pub fn folder(self) -> &'static str {
        match self {
            EffectKind::TailingsSurge => "Tailings Surge",
            EffectKind::ExcavatorSlam => "Excavator Slam",
            EffectKind::AcidPool => "Acid Pool",
            EffectKind::DebrisShadow => "Debris Rain Shadows",
            EffectKind::DebrisImpact => "Debris Rain Impacts",
            EffectKind::QuicksilverWave => "Quicksilver Wave",
            EffectKind::MercuryPool => "Mercury Pool",
            EffectKind::MadnessSpray => "Madness Spray",
            EffectKind::Wisp => "Wisp",
            EffectKind::AmalgamationBlast => "Amalgamation Blast",
            EffectKind::Decoy => "Mercuril-spritesheet",
        }
    }

    /// The full asset path of this effect's sheet.
    pub fn sheet_path(self) -> String {
        match self {
            // The decoy borrows the boss's rest pose rather than an effect sheet.
            EffectKind::Decoy => {
                "sprite_packs/Mercuril-spritesheet/iso_idle_down_right/spritesheet.png".to_string()
            }
            other => format!("sprite_packs/Effects/{}/spritesheet.png", other.folder()),
        }
    }

    /// How many frames the sheet's clip plays through.
    pub fn frames(self) -> usize {
        match self {
            EffectKind::MercuryPool => 1,
            _ => 25,
        }
    }

    /// Whether the clip cycles. Static and one-shot effects hold their final
    /// frame instead.
    pub fn loops(self) -> bool {
        matches!(
            self,
            EffectKind::AcidPool
                | EffectKind::QuicksilverWave
                | EffectKind::MadnessSpray
                | EffectKind::Wisp
                | EffectKind::AmalgamationBlast
                | EffectKind::Decoy
        )
    }

    /// Seconds per frame. Impacts run faster so the flare/dust reads while the
    /// hitbox is still live.
    pub fn frame_seconds(self) -> f32 {
        match self {
            // The caustic trail dries up over exactly its lifetime, so its last
            // (empty) frame lands as the hazard despawns instead of looping back
            // to a full puddle.
            EffectKind::TailingsSurge => SURGE_TRAIL_LIFE / self.frames() as f32,
            EffectKind::ExcavatorSlam | EffectKind::DebrisImpact => 0.05,
            _ => 0.093,
        }
    }

    /// The world size the effect is drawn at.
    ///
    /// This is the single source of truth for the sprite; the attack's damage
    /// footprint is derived separately (and shrunk by [`crate::constants::HITBOX_SHRINK`]).
    pub fn world_size(self, phase: u8) -> Vec2 {
        let enraged = phase >= 2;
        match self {
            EffectKind::TailingsSurge => Vec2::splat(if enraged {
                SURGE_TRAIL_WIDTH_P2
            } else {
                SURGE_TRAIL_WIDTH
            }),
            EffectKind::ExcavatorSlam => {
                Vec2::splat(if enraged { SLAM_RADIUS_P2 } else { SLAM_RADIUS } * 2.0)
            }
            EffectKind::AcidPool => Vec2::splat(ACID_POOL_RADIUS * 2.0),
            EffectKind::DebrisShadow => Vec2::splat(DEBRIS_SHADOW_RADIUS * 2.0),
            EffectKind::DebrisImpact => Vec2::splat(DEBRIS_IMPACT_RADIUS * 2.0),
            EffectKind::QuicksilverWave => Vec2::new(WAVE_WIDTH, WAVE_LENGTH),
            EffectKind::MercuryPool => Vec2::splat(MERCURY_POOL_RADIUS * 2.0),
            EffectKind::MadnessSpray => Vec2::splat(WISP_SPRITE_SIZE),
            EffectKind::Wisp => Vec2::splat(WISP_SPRITE_SIZE),
            EffectKind::AmalgamationBlast => Vec2::splat(AMALGAMATION_RADIUS * 2.0),
            EffectKind::Decoy => Vec2::splat(DECOY_SIZE),
        }
    }

    /// Draw layer. Floor effects sit under the boss; airborne effects pass over
    /// it.
    pub fn z(self) -> f32 {
        match self {
            EffectKind::QuicksilverWave
            | EffectKind::MadnessSpray
            | EffectKind::Wisp
            | EffectKind::Decoy => crate::constants::FLOATING_EFFECT_Z,
            _ => crate::constants::GROUND_EFFECT_Z,
        }
    }

    /// How much larger the graphic is drawn than its hitbox. Only the tiny
    /// spray droplets need it; everything else draws at its true size.
    pub fn graphic_scale(self) -> f32 {
        match self {
            EffectKind::MadnessSpray | EffectKind::Wisp => crate::constants::SPRAY_GRAPHIC_SCALE,
            _ => 1.0,
        }
    }
}

impl AttackKind {
    /// The effect shown once the attack is live.
    pub fn effect(self) -> EffectKind {
        match self {
            AttackKind::SurgeTrail => EffectKind::TailingsSurge,
            AttackKind::AcidPool => EffectKind::AcidPool,
            AttackKind::Slam => EffectKind::ExcavatorSlam,
            AttackKind::Debris => EffectKind::DebrisImpact,
            AttackKind::MercuryPool => EffectKind::MercuryPool,
            AttackKind::Decoy => EffectKind::Decoy,
            AttackKind::Wave => EffectKind::QuicksilverWave,
            AttackKind::Spray => EffectKind::MadnessSpray,
            AttackKind::Wisp => EffectKind::Wisp,
            AttackKind::Amalgam => EffectKind::AmalgamationBlast,
        }
    }

    /// The effect shown while the attack winds up. Falling attacks telegraph
    /// with a ground shadow; everything else shows its live art.
    pub fn windup_effect(self) -> EffectKind {
        match self {
            AttackKind::Slam | AttackKind::Debris => EffectKind::DebrisShadow,
            other => other.effect(),
        }
    }
}

/// Drives one spawned effect's clip. Lives on the attack entity it renders.
#[derive(Component, Reflect, Debug)]
pub struct EffectSprite {
    pub kind: EffectKind,
    pub frame: usize,
    pub frame_timer: Timer,
    /// World size the sprite is drawn at (mirrors the attack's hitbox).
    pub size: Vec2,
}

impl EffectSprite {
    pub fn new(kind: EffectKind, phase: u8) -> Self {
        Self {
            kind,
            frame: 0,
            frame_timer: Timer::from_seconds(kind.frame_seconds(), TimerMode::Repeating),
            size: kind.world_size(phase),
        }
    }

    /// Switches to a different effect from its first frame and re-times the
    /// clock. Used when a windup telegraph hands over to its impact.
    pub fn play(&mut self, kind: EffectKind, phase: u8) {
        self.kind = kind;
        self.frame = 0;
        self.frame_timer = Timer::from_seconds(kind.frame_seconds(), TimerMode::Repeating);
        self.size = kind.world_size(phase);
    }

    /// Advances the clip. Looping effects wrap; one-shots hold their last frame.
    pub fn advance(&mut self, dt: f32) {
        let frames = self.kind.frames();
        if frames <= 1 {
            return;
        }
        self.frame_timer
            .tick(core::time::Duration::from_secs_f32(dt));
        if !self.frame_timer.just_finished() {
            return;
        }
        if self.kind.loops() {
            self.frame = (self.frame + 1) % frames;
        } else if self.frame + 1 < frames {
            self.frame += 1;
        }
    }

    /// The atlas section to show, clamped to the drawn frame count.
    pub fn atlas_index(&self) -> usize {
        self.frame.min(self.kind.frames().saturating_sub(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_attack_maps_to_an_effect() {
        for kind in [
            AttackKind::SurgeTrail,
            AttackKind::AcidPool,
            AttackKind::Slam,
            AttackKind::Debris,
            AttackKind::MercuryPool,
            AttackKind::Decoy,
            AttackKind::Wave,
            AttackKind::Spray,
            AttackKind::Wisp,
            AttackKind::Amalgam,
        ] {
            let _ = kind.effect();
            let _ = kind.windup_effect();
        }
    }

    #[test]
    fn falling_attacks_telegraph_with_a_shadow() {
        assert_eq!(AttackKind::Slam.windup_effect(), EffectKind::DebrisShadow);
        assert_eq!(AttackKind::Debris.windup_effect(), EffectKind::DebrisShadow);
        assert_eq!(AttackKind::Slam.effect(), EffectKind::ExcavatorSlam);
        assert_eq!(AttackKind::Debris.effect(), EffectKind::DebrisImpact);
    }

    #[test]
    fn mercury_pool_is_a_single_static_frame() {
        assert_eq!(EffectKind::MercuryPool.frames(), 1);
        assert!(!EffectKind::MercuryPool.loops());
    }

    #[test]
    fn the_caustic_trail_dries_up_once_over_its_lifetime() {
        assert!(
            !EffectKind::TailingsSurge.loops(),
            "the trail must not loop back to a full puddle"
        );
        let duration =
            EffectKind::TailingsSurge.frame_seconds() * EffectKind::TailingsSurge.frames() as f32;
        assert!(
            (duration - SURGE_TRAIL_LIFE).abs() < 0.001,
            "the clip should end exactly as the hazard despawns: {duration} vs {SURGE_TRAIL_LIFE}"
        );
    }

    #[test]
    fn enraged_sizes_are_never_smaller() {
        for kind in [
            EffectKind::TailingsSurge,
            EffectKind::ExcavatorSlam,
            EffectKind::DebrisShadow,
        ] {
            let p1 = kind.world_size(1);
            let p2 = kind.world_size(2);
            assert!(p2.x >= p1.x && p2.y >= p1.y, "{kind:?}");
        }
    }

    #[test]
    fn a_looping_effect_wraps_and_a_one_shot_holds() {
        let mut looping = EffectSprite::new(EffectKind::Wisp, 1);
        for _ in 0..EffectKind::Wisp.frames() {
            looping.advance(1.0);
        }
        assert_eq!(looping.frame, 0, "wrapping clip returns to the first frame");

        let mut one_shot = EffectSprite::new(EffectKind::ExcavatorSlam, 1);
        for _ in 0..(EffectKind::ExcavatorSlam.frames() * 2) {
            one_shot.advance(1.0);
        }
        assert_eq!(
            one_shot.frame,
            EffectKind::ExcavatorSlam.frames() - 1,
            "one-shot holds its last frame"
        );
    }

    #[test]
    fn playing_a_new_effect_resets_and_resizes() {
        let mut effect = EffectSprite::new(EffectKind::DebrisShadow, 1);
        effect.frame = 7;
        effect.play(EffectKind::DebrisImpact, 1);
        assert_eq!(effect.kind, EffectKind::DebrisImpact);
        assert_eq!(effect.frame, 0);
        assert_eq!(effect.size, EffectKind::DebrisImpact.world_size(1));
    }

    #[test]
    fn decoy_borrows_the_boss_idle_sheet() {
        assert_eq!(
            EffectKind::Decoy.sheet_path(),
            "sprite_packs/Mercuril-spritesheet/iso_idle_down_right/spritesheet.png"
        );
    }

    #[test]
    fn ground_effects_sit_below_the_boss_and_floating_ones_above() {
        let boss_z = 0.0;
        for kind in [
            EffectKind::TailingsSurge,
            EffectKind::AcidPool,
            EffectKind::ExcavatorSlam,
            EffectKind::DebrisShadow,
            EffectKind::DebrisImpact,
            EffectKind::MercuryPool,
            EffectKind::AmalgamationBlast,
        ] {
            assert!(kind.z() < boss_z, "{kind:?} should sit under the boss");
        }
        for kind in [
            EffectKind::QuicksilverWave,
            EffectKind::MadnessSpray,
            EffectKind::Wisp,
            EffectKind::Decoy,
        ] {
            assert!(kind.z() > boss_z, "{kind:?} should float over the boss");
        }
    }

    #[test]
    fn only_the_spray_droplets_draw_larger_than_their_hitbox() {
        assert!(EffectKind::MadnessSpray.graphic_scale() > 1.0);
        assert!(EffectKind::Wisp.graphic_scale() > 1.0);
        for kind in [
            EffectKind::TailingsSurge,
            EffectKind::ExcavatorSlam,
            EffectKind::AcidPool,
            EffectKind::MercuryPool,
            EffectKind::QuicksilverWave,
            EffectKind::AmalgamationBlast,
            EffectKind::Decoy,
        ] {
            assert_eq!(kind.graphic_scale(), 1.0, "{kind:?}");
        }
    }
}
