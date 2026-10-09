use crate::components::gear::{GearRecipe, ItemCost, MaterialType};
use crate::components::pot::CropType;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Resource, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct Inventory {
    pub crops: HashMap<CropType, u32>,
    pub materials: HashMap<MaterialType, u32>,
}

impl Inventory {
    pub fn crop_count(&self, crop: CropType) -> u32 {
        self.crops.get(&crop).copied().unwrap_or(0)
    }

    pub fn material_count(&self, material: MaterialType) -> u32 {
        self.materials.get(&material).copied().unwrap_or(0)
    }

    pub fn count(&self, cost: &ItemCost) -> u32 {
        match cost {
            ItemCost::Crop(crop) => self.crop_count(*crop),
            ItemCost::Material(material) => self.material_count(*material),
        }
    }

    pub fn add_crop(&mut self, crop: CropType, amount: u32) {
        *self.crops.entry(crop).or_insert(0) += amount;
    }

    pub fn add_material(&mut self, material: MaterialType, amount: u32) {
        *self.materials.entry(material).or_insert(0) += amount;
    }

    pub fn missing(&self, recipe: &GearRecipe) -> Vec<(ItemCost, u32)> {
        recipe
            .cost
            .iter()
            .filter_map(|(item, required)| {
                let shortage = required.saturating_sub(self.count(item));
                (shortage > 0).then_some((*item, shortage))
            })
            .collect()
    }

    pub fn can_craft(&self, recipe: &GearRecipe) -> bool {
        self.missing(recipe).is_empty()
    }

    pub fn consume(&mut self, recipe: &GearRecipe) -> bool {
        if !self.can_craft(recipe) {
            return false;
        }
        for (item, required) in recipe.cost {
            match *item {
                ItemCost::Crop(crop) => {
                    let entry = self.crops.entry(crop).or_insert(0);
                    *entry = entry.saturating_sub(*required);
                }
                ItemCost::Material(material) => {
                    let entry = self.materials.entry(material).or_insert(0);
                    *entry = entry.saturating_sub(*required);
                }
            }
        }
        true
    }

    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = CropType::ALL
            .iter()
            .map(|crop| format!("{} {}", crop.label(), self.crop_count(*crop)))
            .collect();
        parts.extend(
            MaterialType::ALL
                .iter()
                .map(|material| format!("{} {}", material.label(), self.material_count(*material))),
        );
        parts.join("  |  ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::gear::{GearPiece, GearSet, GearSlot, RECIPES, recipe_for_piece};

    fn weapon(set: GearSet) -> GearPiece {
        GearPiece::new(set, GearSlot::Weapon)
    }

    fn armor(set: GearSet) -> GearPiece {
        GearPiece::new(set, GearSlot::Armor)
    }

    fn inventory_with_crops(crops: &[(CropType, u32)]) -> Inventory {
        let mut inventory = Inventory::default();
        for (crop, amount) in crops {
            inventory.add_crop(*crop, *amount);
        }
        inventory
    }

    fn inventory_with_materials(materials: &[(MaterialType, u32)]) -> Inventory {
        let mut inventory = Inventory::default();
        for (material, amount) in materials {
            inventory.add_material(*material, *amount);
        }
        inventory
    }

    #[test]
    fn inventory_starts_empty() {
        let inventory = Inventory::default();
        assert!(inventory.crops.is_empty());
        assert!(inventory.materials.is_empty());
        assert_eq!(inventory.crop_count(CropType::Starter), 0);
        assert_eq!(inventory.material_count(MaterialType::BossA1), 0);
    }

    #[test]
    fn add_crop_accumulates() {
        let mut inventory = Inventory::default();
        inventory.add_crop(CropType::Starter, 3);
        inventory.add_crop(CropType::Starter, 4);
        assert_eq!(inventory.crop_count(CropType::Starter), 7);
        assert_eq!(inventory.crop_count(CropType::CropA), 0);
    }

    #[test]
    fn add_material_accumulates_per_type() {
        let mut inventory = Inventory::default();
        inventory.add_material(MaterialType::BossA1, 2);
        inventory.add_material(MaterialType::BossB2, 5);
        assert_eq!(inventory.material_count(MaterialType::BossA1), 2);
        assert_eq!(inventory.material_count(MaterialType::BossB2), 5);
    }

    #[test]
    fn count_dispatches_on_cost_kind() {
        let mut inventory = Inventory::default();
        inventory.add_crop(CropType::CropA, 6);
        inventory.add_material(MaterialType::BossA1, 1);

        assert_eq!(inventory.count(&ItemCost::Crop(CropType::CropA)), 6);
        assert_eq!(
            inventory.count(&ItemCost::Material(MaterialType::BossA1)),
            1
        );
        assert_eq!(inventory.count(&ItemCost::Crop(CropType::CropB)), 0);
    }

    #[test]
    fn empty_inventory_cannot_craft_anything() {
        let inventory = Inventory::default();
        for recipe in RECIPES.iter() {
            assert!(!inventory.can_craft(recipe));
        }
    }

    #[test]
    fn can_craft_starter_weapon_at_exact_cost() {
        let inventory = inventory_with_crops(&[(CropType::Starter, 2)]);
        assert!(inventory.can_craft(recipe_for_piece(weapon(GearSet::Starter))));
    }

    #[test]
    fn can_craft_one_short_is_false() {
        let inventory = inventory_with_crops(&[(CropType::Starter, 1)]);
        assert!(!inventory.can_craft(recipe_for_piece(weapon(GearSet::Starter))));
    }

    #[test]
    fn missing_reports_shortfall_per_item() {
        let inventory = inventory_with_crops(&[(CropType::CropA, 1)]);
        let missing = inventory.missing(recipe_for_piece(weapon(GearSet::BossA)));
        assert_eq!(missing.len(), 2);
        assert!(missing.contains(&(ItemCost::Crop(CropType::CropA), 1)));
        assert!(missing.contains(&(ItemCost::Material(MaterialType::BossA1), 1)));
    }

    #[test]
    fn missing_is_empty_when_affordable() {
        let mut inventory = inventory_with_crops(&[(CropType::CropA, 2)]);
        inventory.add_material(MaterialType::BossA1, 1);
        assert!(
            inventory
                .missing(recipe_for_piece(weapon(GearSet::BossA)))
                .is_empty()
        );
    }

    #[test]
    fn consume_deducts_every_cost_line() {
        let mut inventory = inventory_with_crops(&[(CropType::CropA, 4)]);
        inventory.add_material(MaterialType::BossA1, 2);

        assert!(inventory.consume(recipe_for_piece(weapon(GearSet::BossA))));
        assert_eq!(inventory.crop_count(CropType::CropA), 2);
        assert_eq!(inventory.material_count(MaterialType::BossA1), 1);
    }

    #[test]
    fn consume_fails_and_keeps_inventory_when_short() {
        let mut inventory = inventory_with_crops(&[(CropType::Starter, 1)]);
        let before = inventory.clone();

        assert!(!inventory.consume(recipe_for_piece(weapon(GearSet::Starter))));
        assert_eq!(inventory.crop_count(CropType::Starter), 1);
        assert_eq!(inventory.crops, before.crops);
        assert_eq!(inventory.materials, before.materials);
    }

    #[test]
    fn consume_does_not_partially_spend_a_multi_item_recipe() {
        let mut inventory = inventory_with_crops(&[(CropType::CropA, 2)]);
        assert!(!inventory.consume(recipe_for_piece(weapon(GearSet::BossA))));
        assert_eq!(inventory.crop_count(CropType::CropA), 2);
        assert_eq!(inventory.material_count(MaterialType::BossA1), 0);
    }

    #[test]
    fn starter_weapon_cannot_be_crafted_twice_from_exact_stock() {
        let mut inventory = inventory_with_crops(&[(CropType::Starter, 2)]);
        assert!(inventory.consume(recipe_for_piece(weapon(GearSet::Starter))));
        assert!(!inventory.can_craft(recipe_for_piece(weapon(GearSet::Starter))));
    }

    #[test]
    fn consume_leaves_unrelated_items_untouched() {
        let mut inventory = inventory_with_crops(&[(CropType::Starter, 5), (CropType::CropB, 4)]);
        assert!(inventory.consume(recipe_for_piece(armor(GearSet::Starter))));
        assert_eq!(inventory.crop_count(CropType::CropB), 4);
    }

    #[test]
    fn master_weapon_needs_every_crop_and_both_material_ones() {
        let inventory = inventory_with_crops(&[
            (CropType::Starter, 1),
            (CropType::CropA, 1),
            (CropType::CropB, 1),
        ]);
        let recipe = recipe_for_piece(weapon(GearSet::Master));
        assert!(!inventory.can_craft(recipe));

        let mut materials = inventory_with_materials(&[(MaterialType::BossA1, 2)]);
        for (crop, amount) in inventory.crops.clone() {
            materials.add_crop(crop, amount);
        }
        assert!(!materials.can_craft(recipe));

        materials.add_material(MaterialType::BossB1, 2);
        assert!(materials.can_craft(recipe));
    }

    #[test]
    fn summary_lists_every_crop_and_material() {
        let mut inventory = Inventory::default();
        inventory.add_crop(CropType::Starter, 7);
        inventory.add_material(MaterialType::BossB1, 1);

        let summary = inventory.summary();
        for crop in CropType::ALL {
            assert!(summary.contains(crop.label()), "missing {}", crop.label());
        }
        for material in MaterialType::ALL {
            assert!(
                summary.contains(material.label()),
                "missing {}",
                material.label()
            );
        }
        assert!(summary.contains("Quicksilver Reed 7"));
        assert!(summary.contains("Mercurial Spike 1"));
    }
}
