use crate::components::boss::{BossId, PatternType};
use bevy::prelude::*;

#[derive(Message, Debug, Clone)]
pub struct InteractionEvent {
    pub entity: Entity,
    pub interaction_type: InteractionType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionType {
    FarmAction,
    Crafting,
    BossArena,
    NPC,
}

#[derive(Message, Debug, Clone)]
pub struct DayAdvanced {
    pub day: u32,
}

#[derive(Message, Debug, Clone)]
pub struct CropPlanted(pub crate::components::pot::CropType);

#[derive(Message, Debug, Clone)]
pub struct CropWatered;

#[derive(Message, Debug, Clone)]
pub struct CropHarvested(pub crate::components::pot::CropType);

#[derive(Message, Debug, Clone, Copy)]
pub struct GearCrafted(pub crate::components::gear::GearPiece);

#[derive(Message, Debug, Clone, Copy)]
pub struct GearEquipped(pub crate::components::gear::GearSlot);

/// Emitted when the player confirms a boss in the selection menu.
#[derive(Message, Debug, Clone, Copy)]
pub struct BossSelected(pub BossId);

/// Emitted from combat when a boss runs out of health.
#[derive(Message, Debug, Clone, Copy)]
pub struct BossDefeated(pub BossId);

/// Emitted from combat when the player's health is depleted.
#[derive(Message, Debug, Clone, Copy)]
pub struct PlayerDied;

/// Emitted when a boss crosses into its next phase.
#[derive(Message, Debug, Clone, Copy)]
pub struct BossPhaseChanged {
    pub boss_id: BossId,
    pub new_phase: u8,
}

/// Emitted whenever a hit lands, after armor is applied.
#[derive(Message, Debug, Clone, Copy)]
pub struct DamageDealt {
    pub target: Entity,
    /// Damage actually taken.
    pub amount: f32,
    /// Damage before armor.
    pub raw: f32,
}

/// Emitted with the world position of an impact, for hit feedback effects.
#[derive(Message, Debug, Clone, Copy)]
pub struct HitConfirm {
    pub target: Entity,
    pub position: Vec2,
}

/// Emitted when a boss commits to an attack pattern. Pattern systems listen for
/// this and spawn the matching hazards and projectiles.
#[derive(Message, Debug, Clone, Copy)]
pub struct BossAttackStarted {
    /// The boss entity that threw the pattern.
    pub entity: Entity,
    pub boss_id: BossId,
    pub pattern: PatternType,
    pub phase: u8,
}

/// A sound effect the game wants to play. Kept as an id so the audio layer can
/// resolve it to a file (or stay silent) independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sfx {
    // Boss A: The Excavator
    SurgeWindup,
    SurgeCharge,
    TailingsSizzle,
    SlamWindup,
    SlamImpact,
    AcidForm,
    DebrisWarning,
    DebrisImpact,
    ExcavatorPhase2,
    // Boss B: The Quicksilver
    Blink,
    DecoySpawn,
    DecoyPop,
    WaveLaunch,
    SprayWindup,
    SprayRelease,
    MadnessApply,
    QuicksilverPhase2,
    // Dual
    AmalgamWarning,
    AmalgamChannel,
    AmalgamExplode,
    // Shared
    BossHit,
    PlayerHit,
    BossDefeated,
    MaterialDrop,
}

impl Sfx {
    pub const ALL: [Sfx; 24] = [
        Sfx::SurgeWindup,
        Sfx::SurgeCharge,
        Sfx::TailingsSizzle,
        Sfx::SlamWindup,
        Sfx::SlamImpact,
        Sfx::AcidForm,
        Sfx::DebrisWarning,
        Sfx::DebrisImpact,
        Sfx::ExcavatorPhase2,
        Sfx::Blink,
        Sfx::DecoySpawn,
        Sfx::DecoyPop,
        Sfx::WaveLaunch,
        Sfx::SprayWindup,
        Sfx::SprayRelease,
        Sfx::MadnessApply,
        Sfx::QuicksilverPhase2,
        Sfx::AmalgamWarning,
        Sfx::AmalgamChannel,
        Sfx::AmalgamExplode,
        Sfx::BossHit,
        Sfx::PlayerHit,
        Sfx::BossDefeated,
        Sfx::MaterialDrop,
    ];

    /// The asset filename this cue resolves to, once the audio lands.
    pub fn file(self) -> &'static str {
        match self {
            Sfx::SurgeWindup => "boss_a_surge_windup.ogg",
            Sfx::SurgeCharge => "boss_a_surge_charge.ogg",
            Sfx::TailingsSizzle => "boss_a_tailings_sizzle.ogg",
            Sfx::SlamWindup => "boss_a_slam_windup.ogg",
            Sfx::SlamImpact => "boss_a_slam_impact.ogg",
            Sfx::AcidForm => "boss_a_acid_form.ogg",
            Sfx::DebrisWarning => "boss_a_debris_warning.ogg",
            Sfx::DebrisImpact => "boss_a_debris_impact.ogg",
            Sfx::ExcavatorPhase2 => "boss_a_phase2.ogg",
            Sfx::Blink => "boss_b_blink.ogg",
            Sfx::DecoySpawn => "boss_b_decoy_spawn.ogg",
            Sfx::DecoyPop => "boss_b_decoy_pop.ogg",
            Sfx::WaveLaunch => "boss_b_wave_launch.ogg",
            Sfx::SprayWindup => "boss_b_spray_windup.ogg",
            Sfx::SprayRelease => "boss_b_spray_release.ogg",
            Sfx::MadnessApply => "boss_b_madness_apply.ogg",
            Sfx::QuicksilverPhase2 => "boss_b_phase2.ogg",
            Sfx::AmalgamWarning => "dual_amalgam_warning.ogg",
            Sfx::AmalgamChannel => "dual_amalgam_channel.ogg",
            Sfx::AmalgamExplode => "dual_amalgam_explode.ogg",
            Sfx::BossHit => "boss_hit.ogg",
            Sfx::PlayerHit => "player_hit_boss.ogg",
            Sfx::BossDefeated => "boss_defeated.ogg",
            Sfx::MaterialDrop => "material_drop.ogg",
        }
    }
}

/// Request to play a sound effect.
#[derive(Message, Debug, Clone, Copy)]
pub struct PlaySfx(pub Sfx);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interaction_event_creation() {
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();
        let event = InteractionEvent {
            entity,
            interaction_type: InteractionType::FarmAction,
        };
        assert_eq!(event.entity, entity);
        assert_eq!(event.interaction_type, InteractionType::FarmAction);
    }

    #[test]
    fn day_advanced_event_creation() {
        let event = DayAdvanced { day: 5 };
        assert_eq!(event.day, 5);
    }

    #[test]
    fn gear_messages_creation() {
        let piece = crate::components::gear::GearPiece::new(
            crate::components::gear::GearSet::Starter,
            crate::components::gear::GearSlot::Weapon,
        );
        let crafted = GearCrafted(piece);
        assert_eq!(crafted.0, piece);
        assert_eq!(
            GearEquipped(piece.slot).0,
            crate::components::gear::GearSlot::Weapon
        );
    }

    #[test]
    fn combat_messages_carry_their_payload() {
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();

        let phase = BossPhaseChanged {
            boss_id: crate::components::boss::BossId::BossB,
            new_phase: 2,
        };
        assert_eq!(phase.boss_id, crate::components::boss::BossId::BossB);
        assert_eq!(phase.new_phase, 2);

        let damage = DamageDealt {
            target: entity,
            amount: 12.0,
            raw: 22.0,
        };
        assert_eq!(damage.target, entity);
        assert_eq!(damage.amount, 12.0);
        assert_eq!(damage.raw, 22.0);
    }
}
