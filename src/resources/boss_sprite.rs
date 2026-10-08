use crate::components::boss_animation::{BossAnimState, BossPack};
use crate::components::player_sprite::{FRAME_COLUMNS, FRAME_ROWS, FRAME_SIZE, Facing8};
use bevy::prelude::*;
use std::collections::HashMap;

/// Identifies one boss animation clip: a pack, a clip and a direction.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BossSpriteKey {
    pub pack: BossPack,
    pub state: BossAnimState,
    pub facing: Facing8,
}

impl BossSpriteKey {
    pub fn new(pack: BossPack, state: BossAnimState, facing: Facing8) -> Self {
        Self {
            pack,
            state,
            facing,
        }
    }

    pub fn path(self) -> String {
        self.state.sheet_path(self.pack, self.facing)
    }
}

/// Lazily loaded boss sprite sheets.
///
/// The full set is far too large to decode up front, so a sheet is loaded the
/// first time a `(pack, clip, direction)` is needed and then kept. One shared
/// grid layout serves every sheet because they all use the same frame size.
#[derive(Resource, Default)]
pub struct BossSpriteAssets {
    layout: Option<Handle<TextureAtlasLayout>>,
    sheets: HashMap<BossSpriteKey, Handle<Image>>,
}

impl BossSpriteAssets {
    pub fn layout(&self) -> Option<&Handle<TextureAtlasLayout>> {
        self.layout.as_ref()
    }

    pub fn set_layout(&mut self, layout: Handle<TextureAtlasLayout>) {
        self.layout = Some(layout);
    }

    /// The handle for a clip, loading it on first use.
    pub fn image_for(&mut self, key: BossSpriteKey, server: &AssetServer) -> Handle<Image> {
        self.sheets
            .entry(key)
            .or_insert_with(|| server.load(key.path()))
            .clone()
    }

    pub fn loaded_count(&self) -> usize {
        self.sheets.len()
    }

    /// Drops every cached handle so sheets the next scene does not need can be
    /// unloaded once the scene manifest stops referencing them.
    pub fn clear(&mut self) {
        self.sheets.clear();
    }

    /// The shared 5x5 atlas grid.
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
        let layout = BossSpriteAssets::grid_layout();
        assert_eq!(layout.size, UVec2::splat(FRAME_SIZE * FRAME_COLUMNS));
        assert_eq!((FRAME_COLUMNS * FRAME_ROWS) as usize, layout.len());
    }

    #[test]
    fn sprite_keys_describe_their_sheet_path() {
        let key = BossSpriteKey::new(
            BossPack::Excavator,
            BossAnimState::DebrisRain,
            Facing8::UpRight,
        );
        assert_eq!(
            key.path(),
            "sprite_packs/Excavator-spritesheet/Debris Rain Northeast/spritesheet.png"
        );
    }
}
