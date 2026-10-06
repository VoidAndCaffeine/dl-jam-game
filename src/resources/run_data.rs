use crate::components::gear::{GearPiece, GearSet, GearSlot};
use crate::constants::{
    PLAYER_BASE_HEALTH, UNARMED_DAMAGE, UNARMED_HEAVY_REACH, UNARMED_HEAVY_WIDTH,
    UNARMED_LIGHT_REACH,
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Everything the equipped gear contributes to combat, resolved in one place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerStats {
    pub max_health: f32,
    pub armor_reduction: f32,
    pub weapon_damage: f32,
    pub light_reach: f32,
    pub heavy_reach: f32,
    pub heavy_width: f32,
}

impl PlayerStats {
    /// The bare-handed baseline used when nothing is equipped.
    pub fn unarmed() -> Self {
        Self {
            max_health: PLAYER_BASE_HEALTH,
            armor_reduction: 0.0,
            weapon_damage: UNARMED_DAMAGE,
            light_reach: UNARMED_LIGHT_REACH,
            heavy_reach: UNARMED_HEAVY_REACH,
            heavy_width: UNARMED_HEAVY_WIDTH,
        }
    }
}

#[derive(Resource, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct PlayerGear {
    pub weapon: Option<GearPiece>,
    pub armor: Option<GearPiece>,
    pub owned: Vec<GearPiece>,
}

impl PlayerGear {
    pub fn equipped(&self, slot: GearSlot) -> Option<GearPiece> {
        match slot {
            GearSlot::Weapon => self.weapon,
            GearSlot::Armor => self.armor,
        }
    }

    pub fn slot_mut(&mut self, slot: GearSlot) -> &mut Option<GearPiece> {
        match slot {
            GearSlot::Weapon => &mut self.weapon,
            GearSlot::Armor => &mut self.armor,
        }
    }

    pub fn owns(&self, piece: &GearPiece) -> bool {
        self.owned.contains(piece)
    }

    /// A set counts as owned only once every piece it grants is owned.
    pub fn owns_set(&self, set: GearSet) -> bool {
        set.pieces().iter().all(|piece| self.owns(piece))
    }

    pub fn own(&mut self, piece: GearPiece) -> bool {
        if self.owns(&piece) {
            return false;
        }
        self.owned.push(piece);
        true
    }

    pub fn equip(&mut self, piece: &GearPiece) -> bool {
        if !self.owns(piece) {
            return false;
        }
        if self.equipped(piece.slot) == Some(*piece) {
            return false;
        }
        *self.slot_mut(piece.slot) = Some(*piece);
        true
    }

    pub fn is_equipped(&self, piece: &GearPiece) -> bool {
        self.equipped(piece.slot) == Some(*piece)
    }

    pub fn equipped_summary(&self) -> String {
        GearSlot::ALL
            .iter()
            .map(|slot| match self.equipped(*slot) {
                Some(piece) => format!("{}: {}", slot.label(), piece.name()),
                None => format!("{}: -", slot.label()),
            })
            .collect::<Vec<String>>()
            .join("  |  ")
    }

    /// Resolves the equipped weapon and armor into combat numbers.
    pub fn stats(&self) -> PlayerStats {
        let weapon = self.weapon.map(|piece| piece.set);
        let armor = self.armor.map(|piece| piece.set);
        PlayerStats {
            max_health: armor.map_or(PLAYER_BASE_HEALTH, GearSet::max_health),
            armor_reduction: armor.map_or(0.0, GearSet::armor_reduction),
            weapon_damage: weapon.map_or(UNARMED_DAMAGE, GearSet::weapon_damage),
            light_reach: weapon.map_or(UNARMED_LIGHT_REACH, GearSet::light_reach),
            heavy_reach: weapon.map_or(UNARMED_HEAVY_REACH, GearSet::heavy_reach),
            heavy_width: weapon.map_or(UNARMED_HEAVY_WIDTH, GearSet::heavy_width),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starter_weapon() -> GearPiece {
        GearPiece::new(GearSet::Starter, GearSlot::Weapon)
    }

    fn starter_armor() -> GearPiece {
        GearPiece::new(GearSet::Starter, GearSlot::Armor)
    }

    fn master_weapon() -> GearPiece {
        GearPiece::new(GearSet::Master, GearSlot::Weapon)
    }

    fn gear_owned_from(recipe_index: usize) -> PlayerGear {
        let mut gear = PlayerGear::default();
        for piece in GearSet::ALL[recipe_index].pieces() {
            gear.own(piece);
        }
        gear
    }

    #[test]
    fn player_gear_starts_empty() {
        let gear = PlayerGear::default();
        assert!(gear.weapon.is_none());
        assert!(gear.armor.is_none());
        assert!(gear.owned.is_empty());
        assert!(!gear.owns_set(GearSet::Starter));
    }

    #[test]
    fn own_records_piece_once() {
        let mut gear = PlayerGear::default();
        assert!(gear.own(starter_weapon()));
        assert!(!gear.own(starter_weapon()));
        assert_eq!(gear.owned.len(), 1);
        assert!(gear.owns(&starter_weapon()));
    }

    #[test]
    fn own_does_not_equip() {
        let mut gear = PlayerGear::default();
        gear.own(starter_weapon());
        assert!(gear.weapon.is_none());
    }

    #[test]
    fn equip_requires_ownership() {
        let mut gear = PlayerGear::default();
        assert!(!gear.equip(&starter_weapon()));
        assert!(gear.weapon.is_none());
    }

    #[test]
    fn equip_sets_only_the_matching_slot() {
        let mut gear = gear_owned_from(0);
        assert!(gear.equip(&starter_weapon()));
        assert_eq!(gear.weapon, Some(starter_weapon()));
        assert!(gear.armor.is_none());
    }

    #[test]
    fn equip_reports_false_when_already_equipped() {
        let mut gear = gear_owned_from(0);
        assert!(gear.equip(&starter_weapon()));
        assert!(!gear.equip(&starter_weapon()));
        assert_eq!(gear.weapon, Some(starter_weapon()));
    }

    #[test]
    fn owns_set_needs_every_piece_of_the_recipe() {
        let gear = gear_owned_from(0);
        assert!(gear.owns_set(GearSet::Starter));
        assert!(gear.owns(&starter_weapon()));
        assert!(gear.owns(&starter_armor()));
        assert!(!gear.owns_set(GearSet::BossA));

        let mut partial = PlayerGear::default();
        partial.own(master_weapon());
        assert!(
            !partial.owns_set(GearSet::Master),
            "a set with only one of two pieces is not owned"
        );
        partial.own(GearPiece::new(GearSet::Master, GearSlot::Armor));
        assert!(partial.owns_set(GearSet::Master));
    }

    #[test]
    fn weapons_and_armor_from_different_sets_can_be_mixed() {
        let mut gear = gear_owned_from(0);
        gear.own(master_weapon());
        gear.own(GearPiece::new(GearSet::Master, GearSlot::Armor));

        assert!(gear.equip(&master_weapon()));
        assert!(gear.equip(&starter_armor()));

        assert_eq!(gear.weapon, Some(master_weapon()));
        assert_eq!(gear.armor, Some(starter_armor()));
        assert_eq!(gear.weapon.unwrap().set, GearSet::Master);
        assert_eq!(gear.armor.unwrap().set, GearSet::Starter);
    }

    #[test]
    fn equipping_another_weapon_leaves_armor_alone() {
        let mut gear = gear_owned_from(0);
        gear.own(master_weapon());

        assert!(gear.equip(&starter_weapon()));
        assert!(gear.equip(&starter_armor()));
        assert!(gear.equip(&master_weapon()));

        assert_eq!(gear.weapon, Some(master_weapon()));
        assert_eq!(gear.armor, Some(starter_armor()));
    }

    #[test]
    fn equipping_a_duplicate_piece_is_rejected() {
        let mut gear = gear_owned_from(0);
        assert!(!gear.own(starter_weapon()));
        assert_eq!(gear.owned.len(), 2);
    }

    #[test]
    fn equipped_reads_back_both_slots() {
        let mut gear = gear_owned_from(1);
        assert!(gear.equip(&gear.owned[0].clone()));
        assert!(gear.equip(&gear.owned[1].clone()));
        assert_eq!(gear.equipped(GearSlot::Weapon), Some(gear.owned[0]));
        assert_eq!(gear.equipped(GearSlot::Armor), Some(gear.owned[1]));
        assert!(gear.is_equipped(&gear.owned[0]));
        assert!(!gear.is_equipped(&GearPiece::new(GearSet::Master, GearSlot::Weapon)));
    }

    #[test]
    fn equipped_summary_shows_slots_and_placeholders() {
        let gear = PlayerGear::default();
        let summary = gear.equipped_summary();
        assert!(summary.contains("Weapon: -"));
        assert!(summary.contains("Armor: -"));

        let mut geared = gear_owned_from(0);
        assert!(geared.equip(&starter_weapon()));
        assert!(geared.equip(&starter_armor()));
        let summary = geared.equipped_summary();
        assert!(summary.contains("Weapon: Starter Spearblade"));
        assert!(summary.contains("Armor: Cloth Tunic"));
    }

    #[test]
    fn unarmoured_player_uses_the_baseline_stats() {
        let stats = PlayerGear::default().stats();
        assert_eq!(stats, PlayerStats::unarmed());
        assert_eq!(stats.max_health, PLAYER_BASE_HEALTH);
        assert_eq!(stats.armor_reduction, 0.0);
    }

    #[test]
    fn stats_follow_the_equipped_weapon_and_armor() {
        let mut gear = gear_owned_from(0);
        gear.own(master_weapon());
        gear.own(GearPiece::new(GearSet::Master, GearSlot::Armor));
        assert!(gear.equip(&master_weapon()));
        assert!(gear.equip(&GearPiece::new(GearSet::Master, GearSlot::Armor)));

        let stats = gear.stats();
        assert_eq!(stats.max_health, GearSet::Master.max_health());
        assert_eq!(stats.armor_reduction, GearSet::Master.armor_reduction());
        assert_eq!(stats.weapon_damage, GearSet::Master.weapon_damage());
        assert_eq!(stats.heavy_width, GearSet::Master.heavy_width());
    }

    #[test]
    fn a_weapon_without_armor_keeps_the_base_health() {
        let mut gear = PlayerGear::default();
        gear.own(master_weapon());
        assert!(gear.equip(&master_weapon()));

        let stats = gear.stats();
        assert_eq!(stats.max_health, PLAYER_BASE_HEALTH);
        assert_eq!(stats.armor_reduction, 0.0);
        assert_eq!(stats.weapon_damage, GearSet::Master.weapon_damage());
    }

    #[test]
    fn armor_without_a_weapon_keeps_the_unarmed_damage() {
        let mut gear = PlayerGear::default();
        let armor = GearPiece::new(GearSet::BossA, GearSlot::Armor);
        gear.own(armor);
        assert!(gear.equip(&armor));

        let stats = gear.stats();
        assert_eq!(stats.weapon_damage, UNARMED_DAMAGE);
        assert_eq!(stats.max_health, GearSet::BossA.max_health());
    }
}
