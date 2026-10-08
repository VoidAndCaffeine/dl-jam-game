use crate::components::effect_sprite::EffectKind;
use bevy::prelude::*;
use std::collections::HashMap;

/// Lazily loaded effect sheets.
///
/// The full set is small, but following the player/boss pattern keeps every
/// scene able to drop the effects it no longer needs. Sheets are all 5x5 grids,
/// though not all the same frame size (the trimmed quicksilver wave is shorter),
/// so a layout is built per frame size on demand.
#[derive(Resource, Default)]
pub struct EffectSpriteAssets {
    layouts: HashMap<UVec2, Handle<TextureAtlasLayout>>,
    sheets: HashMap<EffectKind, Handle<Image>>,
}

impl EffectSpriteAssets {
    /// The handle for an effect, loading it on first use.
    pub fn image_for(&mut self, kind: EffectKind, server: &AssetServer) -> Handle<Image> {
        self.sheets
            .entry(kind)
            .or_insert_with(|| server.load(kind.sheet_path()))
            .clone()
    }

    /// The grid layout for a frame size, built once per distinct size.
    pub fn layout_for(&self, frame: UVec2) -> Option<&Handle<TextureAtlasLayout>> {
        self.layouts.get(&frame)
    }

    pub fn set_layout(&mut self, frame: UVec2, layout: Handle<TextureAtlasLayout>) {
        self.layouts.insert(frame, layout);
    }

    pub fn loaded_count(&self) -> usize {
        self.sheets.len()
    }

    /// Drops every cached sheet handle so sheets the next scene does not need
    /// can unload. The tiny layouts are kept.
    pub fn clear(&mut self) {
        self.sheets.clear();
    }

    /// A 5x5 grid layout for `frame`.
    pub fn grid_layout(frame: UVec2) -> TextureAtlasLayout {
        TextureAtlasLayout::from_grid(frame, 5, 5, None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_layout_matches_the_frame_size() {
        let layout = EffectSpriteAssets::grid_layout(UVec2::splat(256));
        assert_eq!(layout.size, UVec2::splat(256 * 5));
        assert_eq!(layout.len(), 25, "one section per frame");
    }

    #[test]
    fn no_sheet_is_loaded_until_it_is_requested() {
        let assets = EffectSpriteAssets::default();
        assert_eq!(assets.loaded_count(), 0);
        assert!(assets.layout_for(UVec2::splat(256)).is_none());
    }
}
