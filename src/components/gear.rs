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
            GearSet::BossA => "Boss A Set",
            GearSet::BossB => "Boss B Set",
            GearSet::Master => "Master Set",
        }
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
    BossA,
    BossB,
}

impl MaterialType {
    pub const ALL: [MaterialType; 2] = [MaterialType::BossA, MaterialType::BossB];

    pub fn label(&self) -> &'static str {
        match self {
            MaterialType::BossA => "Boss A Material",
            MaterialType::BossB => "Boss B Material",
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
    pub fn new(set: GearSet, slot: GearSlot) -> Self {
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

    pub fn name(&self) -> &'static str {
        match (self.set, self.slot) {
            (GearSet::Starter, GearSlot::Weapon) => "Wooden Sword",
            (GearSet::Starter, GearSlot::Armor) => "Cloth Tunic",
            (GearSet::BossA, GearSlot::Weapon) => "Ember Saber",
            (GearSet::BossA, GearSlot::Armor) => "Ember Mail",
            (GearSet::BossB, GearSlot::Weapon) => "Gloom Spear",
            (GearSet::BossB, GearSlot::Armor) => "Gloom Robe",
            (GearSet::Master, GearSlot::Weapon) => "Dreamlayer Blade",
            (GearSet::Master, GearSlot::Armor) => "Dreamlayer Aegis",
        }
    }

    pub fn describe(&self) -> String {
        format!("{} ({})", self.name(), self.set.label())
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GearRecipe {
    pub set: GearSet,
    pub cost: &'static [(ItemCost, u32)],
}

impl GearRecipe {
    pub fn pieces(&self) -> [GearPiece; 2] {
        [
            GearPiece::new(self.set, GearSlot::Weapon),
            GearPiece::new(self.set, GearSlot::Armor),
        ]
    }
}

pub const RECIPES: [GearRecipe; 4] = [
    GearRecipe {
        set: GearSet::Starter,
        cost: &[(ItemCost::Crop(CropType::Starter), 10)],
    },
    GearRecipe {
        set: GearSet::BossA,
        cost: &[
            (ItemCost::Crop(CropType::CropA), 5),
            (ItemCost::Material(MaterialType::BossA), 3),
        ],
    },
    GearRecipe {
        set: GearSet::BossB,
        cost: &[
            (ItemCost::Crop(CropType::CropB), 5),
            (ItemCost::Material(MaterialType::BossB), 3),
        ],
    },
    GearRecipe {
        set: GearSet::Master,
        cost: &[
            (ItemCost::Crop(CropType::Starter), 5),
            (ItemCost::Crop(CropType::CropA), 5),
            (ItemCost::Crop(CropType::CropB), 5),
            (ItemCost::Material(MaterialType::BossA), 2),
            (ItemCost::Material(MaterialType::BossB), 2),
        ],
    },
];

pub const RECIPE_COUNT: usize = RECIPES.len();

pub fn recipe_for_set(set: GearSet) -> &'static GearRecipe {
    RECIPES
        .iter()
        .find(|recipe| recipe.set == set)
        .expect("every gear set has a recipe")
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
    fn material_type_default_is_boss_a() {
        assert_eq!(MaterialType::default(), MaterialType::BossA);
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
        assert_eq!(MaterialType::ALL.len(), 2);
        assert!(MaterialType::ALL.contains(&MaterialType::BossA));
        assert!(MaterialType::ALL.contains(&MaterialType::BossB));
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
    fn there_is_a_recipe_per_set() {
        assert_eq!(RECIPES.len(), GearSet::ALL.len());
        for set in GearSet::ALL {
            let recipe = recipe_for_set(set);
            assert_eq!(recipe.set, set);
            assert_eq!(RECIPE_COUNT, 4);
        }
    }

    #[test]
    fn each_recipe_yields_one_weapon_and_one_armor() {
        for recipe in RECIPES.iter() {
            let pieces = recipe.pieces();
            assert_eq!(pieces.len(), 2);
            assert_eq!(pieces[0].slot, GearSlot::Weapon);
            assert_eq!(pieces[1].slot, GearSlot::Armor);
            for piece in pieces {
                assert_eq!(piece.set, recipe.set);
            }
        }
    }

    #[test]
    fn starter_recipe_costs_ten_starter_crops() {
        let recipe = recipe_for_set(GearSet::Starter);
        assert_eq!(recipe.cost.len(), 1);
        assert_eq!(recipe.cost[0], (ItemCost::Crop(CropType::Starter), 10));
    }

    #[test]
    fn boss_recipes_cost_crop_plus_material() {
        let recipe_a = recipe_for_set(GearSet::BossA);
        assert_eq!(recipe_a.cost.len(), 2);
        assert!(
            recipe_a
                .cost
                .contains(&(ItemCost::Crop(CropType::CropA), 5))
        );
        assert!(
            recipe_a
                .cost
                .contains(&(ItemCost::Material(MaterialType::BossA), 3))
        );

        let recipe_b = recipe_for_set(GearSet::BossB);
        assert_eq!(recipe_b.cost.len(), 2);
        assert!(
            recipe_b
                .cost
                .contains(&(ItemCost::Crop(CropType::CropB), 5))
        );
        assert!(
            recipe_b
                .cost
                .contains(&(ItemCost::Material(MaterialType::BossB), 3))
        );
    }

    #[test]
    fn master_recipe_costs_all_crops_and_all_materials() {
        let recipe = recipe_for_set(GearSet::Master);
        assert_eq!(recipe.cost.len(), 5);
        for crop in [CropType::Starter, CropType::CropA, CropType::CropB] {
            assert!(recipe.cost.contains(&(ItemCost::Crop(crop), 5)));
        }
        for material in MaterialType::ALL {
            assert!(recipe.cost.contains(&(ItemCost::Material(material), 2)));
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
    fn recipes_are_ordered_by_gear_set() {
        for (recipe, set) in RECIPES.iter().zip(GearSet::ALL) {
            assert_eq!(recipe.set, set);
        }
    }

    #[test]
    fn item_cost_labels_are_distinct_and_non_empty() {
        let labels = [
            ItemCost::Crop(CropType::Starter).label(),
            ItemCost::Crop(CropType::CropA).label(),
            ItemCost::Crop(CropType::CropB).label(),
            ItemCost::Material(MaterialType::BossA).label(),
            ItemCost::Material(MaterialType::BossB).label(),
        ];
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
}
