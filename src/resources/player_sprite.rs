use crate::components::player_sprite::{
    FRAME_COLUMNS, FRAME_ROWS, FRAME_SIZE, Facing8, PlayerAnimState, PlayerLook,
};
use bevy::prelude::*;
use std::collections::HashMap;

/// Identifies one animation clip: a look, a clip and a direction.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SpriteKey {
    pub look: PlayerLook,
    pub state: PlayerAnimState,
    pub facing: Facing8,
}

impl SpriteKey {
    pub fn new(look: PlayerLook, state: PlayerAnimState, facing: Facing8) -> Self {
        Self {
            look,
            state,
            facing,
        }
    }

    pub fn path(self) -> String {
        self.state.sheet_path(self.look, self.facing)
    }
}

/// Lazily loaded player sprite sheets.
///
/// The full art set is far too large to decode up front (~1.4 GB across all
/// looks, clips and directions), so sheets are loaded the first time a
/// `(look, clip, direction)` is needed and then kept. One shared grid layout
/// serves every sheet because they all have identical dimensions.
#[derive(Resource, Default)]
pub struct PlayerSpriteAssets {
    layout: Option<Handle<TextureAtlasLayout>>,
    sheets: HashMap<SpriteKey, Handle<Image>>,
}

impl PlayerSpriteAssets {
    /// The shared 5x5 atlas layout, once the asset server has supplied one.
    pub fn layout(&self) -> Option<&Handle<TextureAtlasLayout>> {
        self.layout.as_ref()
    }

    /// Stores the shared layout so `Sprite` can reference it.
    pub fn set_layout(&mut self, layout: Handle<TextureAtlasLayout>) {
        self.layout = Some(layout);
    }

    /// The handle for a clip, loading it on first use.
    pub fn image_for(&mut self, key: SpriteKey, server: &AssetServer) -> Handle<Image> {
        self.sheets
            .entry(key)
            .or_insert_with(|| server.load(key.path()))
            .clone()
    }

    /// How many sheets have been requested so far.
    pub fn loaded_count(&self) -> usize {
        self.sheets.len()
    }

    /// Drops every cached handle so sheets the next scene does not need can be
    /// unloaded once the scene manifest stops referencing them.
    pub fn clear(&mut self) {
        self.sheets.clear();
    }

    /// The shared atlas grid for the 5x5 sheets.
    pub fn grid_layout() -> TextureAtlasLayout {
        TextureAtlasLayout::from_grid(
            UVec2::splat(FRAME_SIZE),
            FRAME_COLUMNS,
            FRAME_ROWS,
            None,
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_layout_matches_the_sheets() {
        let layout = PlayerSpriteAssets::grid_layout();
        assert_eq!(layout.size, UVec2::splat(FRAME_SIZE * FRAME_COLUMNS));
        assert_eq!(
            layout.len(),
            (FRAME_COLUMNS * FRAME_ROWS) as usize,
            "one section per frame"
        );
    }

    #[test]
    fn a_missing_look_has_no_layout_until_one_is_set() {
        let mut assets = PlayerSpriteAssets::default();
        assert!(assets.layout().is_none());
        let handle = Handle::<TextureAtlasLayout>::default();
        assets.set_layout(handle);
        assert!(assets.layout().is_some());
    }

    #[test]
    fn sprite_keys_describe_their_sheet_path() {
        let key = SpriteKey::new(PlayerLook::BossA, PlayerAnimState::Walk, Facing8::Up);
        assert_eq!(
            key.path(),
            "sprite_packs/Ember-Gear-spritesheet/iso_walk_up_right/spritesheet.png"
        );
    }

    #[test]
    fn no_sheet_is_loaded_until_it_is_requested() {
        let assets = PlayerSpriteAssets::default();
        assert_eq!(assets.loaded_count(), 0);
    }
}
