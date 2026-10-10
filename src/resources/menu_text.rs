//! Placeholder copy for the redesigned menus.
//!
//! Everything the player reads in the gear, boss and planting panels is
//! authored here as `&'static str` tables so the text is easy to rewrite
//! without touching layout code. The descriptions are intentionally generic
//! placeholders for now.

use crate::components::boss::BossId;
use crate::components::gear::{GearPiece, GearSet, GearSlot, ItemCost};
use crate::components::pot::CropType;

/// Flavor text for one piece of gear, shown in the right-hand pane of the gear
/// menu.
pub fn gear_description(piece: GearPiece) -> &'static str {
    match (piece.set, piece.slot) {
        (GearSet::Starter, GearSlot::Weapon) => STARTER_WEAPON_TEXT,
        (GearSet::BossA, GearSlot::Weapon) => BOSS_A_WEAPON_TEXT,
        (GearSet::BossB, GearSlot::Weapon) => BOSS_B_WEAPON_TEXT,
        (GearSet::Master, GearSlot::Weapon) => FINAL_WEAPON_TEXT,
        (GearSet::Starter, GearSlot::Armor) => STARTER_ARMOR,
        (GearSet::BossA, GearSlot::Armor) => BOSS_A_ARMOR,
        (GearSet::BossB, GearSlot::Armor) => BOSS_B_ARMOR,
        (GearSet::Master, GearSlot::Armor) => FINAL_ARMOR,
    }
}

/// Per-piece weapon descriptions.
const STARTER_WEAPON_TEXT: &str = "A light weapon, made of reinforced reeds, leather, and iron.";
const BOSS_A_WEAPON_TEXT: &str =
    "A spear making excellent use of the highly flammable Cinder Cap spores.";
const BOSS_B_WEAPON_TEXT: &str = "A weapon of shifting form, although usually a spear.";
const FINAL_WEAPON_TEXT: &str =
    "A reinforced spear taking the best and worst from the other weapons";

/// Per-piece armor descriptions.
const STARTER_ARMOR: &str =
    "A light set of winterized armor made of woven reeds. Just don't go mad.";
const BOSS_A_ARMOR: &str = "An oddly warm set of winterized armor. Stay away from open flames.";
const BOSS_B_ARMOR: &str =
    "A heavy set of winterized armor. Heavy yet flexible, just don't breathe too deeply.";
const FINAL_ARMOR: &str =
    "A perfect set of winterized armor. Taking the best and worst from all other armor.";

/// One line describing what a crop yields when harvested.
///
/// Harvesting currently stocks a single crop of the planted type, so the yield
/// is always one; the phrasing leaves room for a richer table later.
pub fn crop_yield(crop: CropType) -> String {
    format!("Yields: {} x1", crop.label())
}

/// The materials a boss can drop, phrased for a boss-select button, e.g.
/// `"Drops: Boss A Material 1 x0-2, Boss A Material 2 x1-3"`. The dual boss has
/// no drops of its own, so it reports that beating it ends the run instead.
pub fn boss_drops(id: BossId) -> String {
    let drops = id.material_drops();
    if drops.is_empty() {
        return "Drops: none - ends the run".to_string();
    }
    let parts: Vec<String> = drops
        .iter()
        .map(|(material, range)| format!("{} x{}-{}", material.label(), range.start(), range.end()))
        .collect();
    format!("Drops: {}", parts.join(", "))
}

/// A short "have / need" summary for a recipe cost line.
pub fn cost_line(item: ItemCost, have: u32, need: u32) -> String {
    format!("{}  {}/{}", item.label(), have, need)
}

/// The materials needed to craft a piece, one short line each. Empty for a
/// piece with no recipe (which cannot happen today, but keeps callers safe).
pub fn recipe_cost_lines(
    recipe: &crate::components::gear::GearRecipe,
    have: impl Fn(ItemCost) -> u32,
) -> Vec<String> {
    recipe
        .cost
        .iter()
        .map(|(item, need)| cost_line(*item, have(*item), *need))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::gear::{GearSet, GearSlot, RECIPES, recipe_for_piece};

    #[test]
    fn every_piece_has_some_description() {
        for set in GearSet::ALL {
            for piece in set.pieces() {
                assert!(!gear_description(piece).is_empty());
            }
        }
    }

    #[test]
    fn every_crop_reports_a_yield() {
        for crop in CropType::ALL {
            let line = crop_yield(crop);
            assert!(line.contains(crop.label()), "{line}");
            assert!(line.starts_with("Yields:"), "{line}");
        }
    }

    #[test]
    fn boss_drops_mention_each_material_and_range() {
        let line = boss_drops(BossId::BossA);
        assert!(line.contains("Rusted Spike"), "{line}");
        assert!(line.contains("0-2"), "{line}");
        assert!(line.contains("1-3"), "{line}");
        assert!(!line.contains("Mercurial"), "{line}");
    }

    #[test]
    fn the_dual_boss_reports_no_drops() {
        let line = boss_drops(BossId::Dual);
        assert!(!line.contains("Material"), "{line}");
    }

    #[test]
    fn recipe_cost_lines_carry_have_and_need() {
        let recipe = recipe_for_piece(GearPiece::new(GearSet::Starter, GearSlot::Weapon));
        let lines = recipe_cost_lines(recipe, |_| 1);
        assert_eq!(lines, vec!["Quicksilver Reed  1/2".to_string()]);
    }

    #[test]
    fn there_is_a_description_for_every_recipe_piece() {
        for recipe in RECIPES.iter() {
            assert!(!gear_description(recipe.piece).is_empty());
        }
    }
}
