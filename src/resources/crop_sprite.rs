//! Lazily loaded crop sprite sheets.
//!
//! A crop sheet is only decoded when its stage first appears on a pot, then kept
//! for the scene. Like the player and boss caches, the handles are dropped on a
//! scene change so the sheets the next scene does not need can unload.

use crate::components::crop_sprite::CropStage;
use crate::components::pot::CropType;
use bevy::prelude::*;
use std::collections::HashMap;

/// Identifies one crop sheet: a crop at a growth stage.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CropSpriteKey {
    pub crop: CropType,
    pub stage: CropStage,
}

impl CropSpriteKey {
    pub fn new(crop: CropType, stage: CropStage) -> Self {
        Self { crop, stage }
    }

    pub fn path(self) -> String {
        self.crop.sheet_path(self.stage)
    }
}

/// The crop sheets the farm has decoded so far.
#[derive(Resource, Default)]
pub struct CropSpriteAssets {
    sheets: HashMap<CropSpriteKey, Handle<Image>>,
}

impl CropSpriteAssets {
    /// The handle for a sheet, loading it on first use.
    pub fn image_for(&mut self, key: CropSpriteKey, server: &AssetServer) -> Handle<Image> {
        self.sheets
            .entry(key)
            .or_insert_with(|| server.load(key.path()))
            .clone()
    }

    /// How many sheets have been requested so far.
    pub fn loaded_count(&self) -> usize {
        self.sheets.len()
    }

    /// Drops every cached sheet handle so the next scene can unload them.
    pub fn clear(&mut self) {
        self.sheets.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_sheet_is_loaded_until_it_is_requested() {
        let assets = CropSpriteAssets::default();
        assert_eq!(assets.loaded_count(), 0);
    }

    #[test]
    fn keys_describe_their_sheet_path() {
        let key = CropSpriteKey::new(CropType::CropA, CropStage::Growing);
        assert_eq!(
            key.path(),
            "sprite_packs/Plants/cinder_cap_growing-spritesheet/spritesheet.png"
        );
    }

    #[test]
    fn the_same_key_returns_the_same_path() {
        let a = CropSpriteKey::new(CropType::CropB, CropStage::GrowingLarge);
        let b = CropSpriteKey::new(CropType::CropB, CropStage::GrowingLarge);
        assert_eq!(a, b);
        assert_eq!(a.path(), b.path());
    }
}
