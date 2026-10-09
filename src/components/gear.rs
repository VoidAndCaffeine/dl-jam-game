use crate::components::pot::CropType;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(
    Component, Reflect, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Default, Debug,
)]
pub enum GearSet {
    #[default]
    Starter,
    BossA,
    BossB,
    Master,
}

impl GearSet {
    pub const ALL: [GearSet; 4] = [
        GearSet::Starter,
        GearSet::BossA,
        GearSet::BossB,
        GearSet::Master,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            GearSet::Starter => "Starter Set",
            GearSet::BossA => "Excavator Set",
            GearSet::BossB => "Mercurial Set",
            GearSet::Master => "Master Set",
        }
    }

    /// The weapon and armor pieces that make up this set.
    pub fn pieces(&self) -> [GearPiece; 2] {
        [
            GearPiece::new(*self, GearSlot::Weapon),
            GearPiece::new(*self, GearSlot::Armor),
        ]
    }

    /// Position of this set in the stat tables in [`crate::constants`].
    pub const fn index(self) -> usize {
        match self {
            GearSet::Starter => 0,
            GearSet::BossA => 1,
            GearSet::BossB => 2,
            GearSet::Master => 3,
        }
    }

    /// Damage this set's weapon deals per light hit.
    pub fn weapon_damage(self) -> f32 {
        crate::constants::SET_DAMAGE[self.index()]
    }

    /// Flat damage reduction this set's armor provides.
    pub fn armor_reduction(self) -> f32 {
        crate::constants::SET_ARMOR[self.index()]
    }

    /// Total max health this set's armor provides.
    pub fn max_health(self) -> f32 {
        crate::constants::SET_MAX_HEALTH[self.index()]
    }

    /// Reach of this set's light swing.
    pub fn light_reach(self) -> f32 {
        crate::constants::SET_LIGHT_REACH[self.index()]
    }

    /// Reach of this set's heavy swing.
    pub fn heavy_reach(self) -> f32 {
        crate::constants::SET_HEAVY_REACH[self.index()]
    }

    /// Width of this set's heavy swing.
    pub fn heavy_width(self) -> f32 {
        crate::constants::SET_HEAVY_WIDTH[self.index()]
    }
}

#[derive(
    Component, Reflect, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Default, Debug,
)]
pub enum GearSlot {
    #[default]
    Weapon,
    Armor,
}

impl GearSlot {
    pub const ALL: [GearSlot; 2] = [GearSlot::Weapon, GearSlot::Armor];

    pub fn label(&self) -> &'static str {
        match self {
            GearSlot::Weapon => "Weapon",
            GearSlot::Armor => "Armor",
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub enum WeaponType {
    #[default]
    WoodenSword,
    BossASaber,
    BossBSpear,
    MasterBlade,
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub enum MaterialType {
    #[default]
    BossA1,
    BossA2,
    BossB1,
    BossB2,
}

impl MaterialType {
    pub const ALL: [MaterialType; 4] = [
        MaterialType::BossA1,
        MaterialType::BossA2,
        MaterialType::BossB1,
        MaterialType::BossB2,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            MaterialType::BossA1 => "Rusted Spike",
            MaterialType::BossA2 => "Rusted Plate",
            MaterialType::BossB1 => "Mercurial Spike",
            MaterialType::BossB2 => "Mercurial Plate",
        }
    }
}

#[derive(Reflect, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ItemCost {
    Crop(CropType),
    Material(MaterialType),
}

impl ItemCost {
    pub fn label(&self) -> String {
        match self {
            ItemCost::Crop(crop) => crop.label().to_string(),
            ItemCost::Material(material) => material.label().to_string(),
        }
    }
}

#[derive(Component, Reflect, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct GearPiece {
    pub set: GearSet,
    pub slot: GearSlot,
}

impl GearPiece {
    pub const fn new(set: GearSet, slot: GearSlot) -> Self {
        Self { set, slot }
    }

    pub fn weapon_type(&self) -> Option<WeaponType> {
        if self.slot != GearSlot::Weapon {
            return None;
        }
        Some(match self.set {
            GearSet::Starter => WeaponType::WoodenSword,
            GearSet::BossA => WeaponType::BossASaber,
            GearSet::BossB => WeaponType::BossBSpear,
            GearSet::Master => WeaponType::MasterBlade,
        })
    }

    /// Damage per light hit, or `None` for armor.
    pub fn weapon_damage(&self) -> Option<f32> {
        (self.slot == GearSlot::Weapon).then(|| self.set.weapon_damage())
    }

    /// Flat damage reduction, or `None` for weapons.
    pub fn armor_reduction(&self) -> Option<f32> {
        (self.slot == GearSlot::Armor).then(|| self.set.armor_reduction())
    }

    /// Total max health, or `None` for weapons.
    pub fn max_health(&self) -> Option<f32> {
        (self.slot == GearSlot::Armor).then(|| self.set.max_health())
    }

    /// Reach of this piece's light swing, or `None` for armor.
    pub fn light_reach(&self) -> Option<f32> {
        (self.slot == GearSlot::Weapon).then(|| self.set.light_reach())
    }

    /// Reach of this piece's heavy swing, or `None` for armor.
    pub fn heavy_reach(&self) -> Option<f32> {
        (self.slot == GearSlot::Weapon).then(|| self.set.heavy_reach())
    }

    /// Width of this piece's heavy swing, or `None` for armor.
    pub fn heavy_width(&self) -> Option<f32> {
        (self.slot == GearSlot::Weapon).then(|| self.set.heavy_width())
    }

    pub fn name(&self) -> &'static str {
        match (self.set, self.slot) {
            (GearSet::Starter, GearSlot::Weapon) => "Starter Spearblade",
            (GearSet::Starter, GearSlot::Armor) => "Cloth Tunic",
            (GearSet::BossA, GearSlot::Weapon) => "Spark Spearblade",
            (GearSet::BossA, GearSlot::Armor) => "Ember Mail",
            (GearSet::BossB, GearSlot::Weapon) => "Shadow Spearblade",
            (GearSet::BossB, GearSlot::Armor) => "Gloom Robe",
            (GearSet::Master, GearSlot::Weapon) => "Dreaming Spearblade",
            (GearSet::Master, GearSlot::Armor) => "Layered Aegis",
        }
    }

    pub fn describe(&self) -> String {
        format!("{} ({})", self.name(), self.set.label())
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GearRecipe {
    pub piece: GearPiece,
    pub cost: &'static [(ItemCost, u32)],
}

impl GearRecipe {
    pub fn slot(&self) -> GearSlot {
        self.piece.slot
    }
}

pub const RECIPES: [GearRecipe; 8] = [
    // Starter
    GearRecipe {
        piece: GearPiece::new(GearSet::Starter, GearSlot::Weapon),
        cost: &[(ItemCost::Crop(CropType::Starter), 2)],
    },
    GearRecipe {
        piece: GearPiece::new(GearSet::Starter, GearSlot::Armor),
        cost: &[(ItemCost::Crop(CropType::Starter), 3)],
    },
    // Boss A
    GearRecipe {
        piece: GearPiece::new(GearSet::BossA, GearSlot::Weapon),
        cost: &[
            (ItemCost::Crop(CropType::CropA), 2),
            (ItemCost::Material(MaterialType::BossA1), 1),
        ],
    },
    GearRecipe {
        piece: GearPiece::new(GearSet::BossA, GearSlot::Armor),
        cost: &[
            (ItemCost::Crop(CropType::CropA), 3),
            (ItemCost::Material(MaterialType::BossA2), 2),
        ],
    },
    // Boss B
    GearRecipe {
        piece: GearPiece::new(GearSet::BossB, GearSlot::Weapon),
        cost: &[
            (ItemCost::Crop(CropType::CropB), 2),
            (ItemCost::Material(MaterialType::BossB1), 1),
        ],
    },
    GearRecipe {
        piece: GearPiece::new(GearSet::BossB, GearSlot::Armor),
        cost: &[
            (ItemCost::Crop(CropType::CropB), 3),
            (ItemCost::Material(MaterialType::BossB2), 2),
        ],
    },
    // Master
    GearRecipe {
        piece: GearPiece::new(GearSet::Master, GearSlot::Weapon),
        cost: &[
            (ItemCost::Crop(CropType::Starter), 1),
            (ItemCost::Crop(CropType::CropA), 1),
            (ItemCost::Crop(CropType::CropB), 1),
            (ItemCost::Material(MaterialType::BossA1), 2),
            (ItemCost::Material(MaterialType::BossB1), 2),
        ],
    },
    GearRecipe {
        piece: GearPiece::new(GearSet::Master, GearSlot::Armor),
        cost: &[
            (ItemCost::Crop(CropType::Starter), 3),
            (ItemCost::Crop(CropType::CropA), 3),
            (ItemCost::Crop(CropType::CropB), 3),
            (ItemCost::Material(MaterialType::BossA2), 4),
            (ItemCost::Material(MaterialType::BossB2), 4),
        ],
    },
];

pub const RECIPE_COUNT: usize = RECIPES.len();

pub fn recipe_for_piece(piece: GearPiece) -> &'static GearRecipe {
    RECIPES
        .iter()
        .find(|recipe| recipe.piece == piece)
        .expect("every gear piece has a recipe")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gear_set_default_is_starter() {
        assert_eq!(GearSet::default(), GearSet::Starter);
    }

    #[test]
    fn gear_slot_default_is_weapon() {
        assert_eq!(GearSlot::default(), GearSlot::Weapon);
    }

    #[test]
    fn material_type_default_is_boss_a1() {
        assert_eq!(MaterialType::default(), MaterialType::BossA1);
    }

    #[test]
    fn all_gear_sets_are_listed_once() {
        assert_eq!(GearSet::ALL.len(), 4);
        for set in GearSet::ALL {
            assert_eq!(GearSet::ALL.iter().filter(|s| **s == set).count(), 1);
        }
    }

    #[test]
    fn all_gear_slots_are_listed_once() {
        assert_eq!(GearSlot::ALL.len(), 2);
        assert!(GearSlot::ALL.contains(&GearSlot::Weapon));
        assert!(GearSlot::ALL.contains(&GearSlot::Armor));
    }

    #[test]
    fn all_material_types_are_listed_once() {
        assert_eq!(MaterialType::ALL.len(), 4);
        for material in MaterialType::ALL {
            assert_eq!(
                MaterialType::ALL.iter().filter(|m| **m == material).count(),
                1
            );
        }
    }

    #[test]
    fn piece_names_are_unique_per_set_and_slot() {
        let mut names: Vec<&str> = Vec::new();
        for set in GearSet::ALL {
            for slot in GearSlot::ALL {
                let piece = GearPiece::new(set, slot);
                assert!(!names.contains(&piece.name()), "duplicate {}", piece.name());
                names.push(piece.name());
            }
        }
        assert_eq!(names.len(), 8);
    }

    #[test]
    fn weapon_type_only_for_weapon_slot() {
        for set in GearSet::ALL {
            let weapon = GearPiece::new(set, GearSlot::Weapon);
            assert!(weapon.weapon_type().is_some());
            let armor = GearPiece::new(set, GearSlot::Armor);
            assert_eq!(armor.weapon_type(), None);
        }
    }

    #[test]
    fn weapon_types_are_unique_per_set() {
        let types: Vec<WeaponType> = GearSet::ALL
            .iter()
            .map(|set| {
                GearPiece::new(*set, GearSlot::Weapon)
                    .weapon_type()
                    .unwrap()
            })
            .collect();
        for (index, first) in types.iter().enumerate() {
            for second in types.iter().skip(index + 1) {
                assert_ne!(first, second);
            }
        }
    }

    #[test]
    fn there_is_a_recipe_per_piece() {
        assert_eq!(RECIPES.len(), 8);
        assert_eq!(RECIPE_COUNT, 8);
        for set in GearSet::ALL {
            for piece in set.pieces() {
                let recipe = recipe_for_piece(piece);
                assert_eq!(recipe.piece, piece);
            }
        }
    }

    #[test]
    fn every_recipe_targets_a_single_named_piece() {
        for recipe in RECIPES.iter() {
            assert_eq!(recipe.piece, recipe_for_piece(recipe.piece).piece);
            assert!(!recipe.piece.name().is_empty());
        }
    }

    #[test]
    fn starter_recipes_cost_two_and_three_crops() {
        let weapon = recipe_for_piece(GearPiece::new(GearSet::Starter, GearSlot::Weapon));
        assert_eq!(weapon.cost, &[(ItemCost::Crop(CropType::Starter), 2)]);
        let armor = recipe_for_piece(GearPiece::new(GearSet::Starter, GearSlot::Armor));
        assert_eq!(armor.cost, &[(ItemCost::Crop(CropType::Starter), 3)]);
    }

    #[test]
    fn boss_weapons_cost_two_crops_and_one_material_one() {
        for (crop, material) in [
            (CropType::CropA, MaterialType::BossA1),
            (CropType::CropB, MaterialType::BossB1),
        ] {
            let set = if crop == CropType::CropA {
                GearSet::BossA
            } else {
                GearSet::BossB
            };
            let recipe = recipe_for_piece(GearPiece::new(set, GearSlot::Weapon));
            assert_eq!(
                recipe.cost,
                &[(ItemCost::Crop(crop), 2), (ItemCost::Material(material), 1),]
            );
        }
    }

    #[test]
    fn boss_armor_costs_three_crops_and_two_material_two() {
        for (crop, material) in [
            (CropType::CropA, MaterialType::BossA2),
            (CropType::CropB, MaterialType::BossB2),
        ] {
            let set = if crop == CropType::CropA {
                GearSet::BossA
            } else {
                GearSet::BossB
            };
            let recipe = recipe_for_piece(GearPiece::new(set, GearSlot::Armor));
            assert_eq!(
                recipe.cost,
                &[(ItemCost::Crop(crop), 3), (ItemCost::Material(material), 2),]
            );
        }
    }

    #[test]
    fn master_weapon_costs_every_crop_and_both_material_ones() {
        let recipe = recipe_for_piece(GearPiece::new(GearSet::Master, GearSlot::Weapon));
        assert_eq!(recipe.cost.len(), 5);
        for crop in [CropType::Starter, CropType::CropA, CropType::CropB] {
            assert!(recipe.cost.contains(&(ItemCost::Crop(crop), 1)));
        }
        for material in [MaterialType::BossA1, MaterialType::BossB1] {
            assert!(recipe.cost.contains(&(ItemCost::Material(material), 2)));
        }
    }

    #[test]
    fn master_armor_costs_every_crop_and_both_material_twos() {
        let recipe = recipe_for_piece(GearPiece::new(GearSet::Master, GearSlot::Armor));
        assert_eq!(recipe.cost.len(), 5);
        for crop in [CropType::Starter, CropType::CropA, CropType::CropB] {
            assert!(recipe.cost.contains(&(ItemCost::Crop(crop), 3)));
        }
        for material in [MaterialType::BossA2, MaterialType::BossB2] {
            assert!(recipe.cost.contains(&(ItemCost::Material(material), 4)));
        }
    }

    #[test]
    fn no_recipe_lists_the_same_cost_twice() {
        for recipe in RECIPES.iter() {
            for (index, first) in recipe.cost.iter().enumerate() {
                for second in recipe.cost.iter().skip(index + 1) {
                    assert_ne!(first.0, second.0);
                }
            }
        }
    }

    #[test]
    fn every_recipe_requires_something() {
        for recipe in RECIPES.iter() {
            assert!(!recipe.cost.is_empty());
            for (_, amount) in recipe.cost {
                assert!(*amount > 0);
            }
        }
    }

    #[test]
    fn recipes_are_ordered_by_set_then_slot() {
        let expected: Vec<GearPiece> = GearSet::ALL.iter().flat_map(|set| set.pieces()).collect();
        let actual: Vec<GearPiece> = RECIPES.iter().map(|recipe| recipe.piece).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn item_cost_labels_are_distinct_and_non_empty() {
        let mut labels = vec![
            ItemCost::Crop(CropType::Starter).label(),
            ItemCost::Crop(CropType::CropA).label(),
            ItemCost::Crop(CropType::CropB).label(),
        ];
        labels.extend(
            MaterialType::ALL
                .iter()
                .map(|material| ItemCost::Material(*material).label()),
        );
        for (index, first) in labels.iter().enumerate() {
            assert!(!first.is_empty());
            for second in labels.iter().skip(index + 1) {
                assert_ne!(first, second);
            }
        }
    }

    #[test]
    fn piece_describe_mentions_name_and_set() {
        let piece = GearPiece::new(GearSet::BossB, GearSlot::Armor);
        let described = piece.describe();
        assert!(described.contains(piece.name()));
        assert!(described.contains(GearSet::BossB.label()));
    }

    #[test]
    fn set_indices_are_unique_and_in_order() {
        for (index, set) in GearSet::ALL.iter().enumerate() {
            assert_eq!(set.index(), index);
        }
    }

    #[test]
    fn weapons_scale_damage_and_reach_with_the_set() {
        let damages: Vec<f32> = GearSet::ALL.iter().map(|set| set.weapon_damage()).collect();
        assert!(damages.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(damages[0], crate::constants::SET_DAMAGE[0]);

        for set in GearSet::ALL {
            assert!(set.light_reach() > 0.0);
            assert!(set.heavy_reach() > 0.0);
            assert!(set.heavy_width() > 0.0);
        }
    }

    #[test]
    fn armor_scales_health_and_reduction_with_the_set() {
        let health: Vec<f32> = GearSet::ALL.iter().map(|set| set.max_health()).collect();
        assert!(health.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(health[0], crate::constants::SET_MAX_HEALTH[0]);

        let armor: Vec<f32> = GearSet::ALL
            .iter()
            .map(|set| set.armor_reduction())
            .collect();
        assert!(armor.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn weapon_and_armor_stats_are_slot_gated() {
        let weapon = GearPiece::new(GearSet::Master, GearSlot::Weapon);
        assert_eq!(
            weapon.weapon_damage(),
            Some(GearSet::Master.weapon_damage())
        );
        assert_eq!(weapon.armor_reduction(), None);
        assert_eq!(weapon.max_health(), None);
        assert!(weapon.light_reach().is_some());

        let armor = GearPiece::new(GearSet::Master, GearSlot::Armor);
        assert_eq!(armor.weapon_damage(), None);
        assert_eq!(armor.light_reach(), None);
        assert_eq!(armor.heavy_width(), None);
        assert_eq!(
            armor.armor_reduction(),
            Some(GearSet::Master.armor_reduction())
        );
        assert_eq!(armor.max_health(), Some(GearSet::Master.max_health()));
    }
}
