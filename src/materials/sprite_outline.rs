//! The shared interaction sprite material: a textured quad whose silhouette
//! outline can be lit up to mark it as the current interaction target.
//!
//! Both the pot mounds and the farm props (crafting station, boss door) draw
//! with this material rather than a plain `Sprite`, because a plain sprite has
//! no hook for a custom outline. The outline is produced in `sprite_outline.wgsl`
//! by scanning the texture's alpha for the sprite edge; toggling
//! [`SpriteOutlineParams::highlight`] fades it in and out, so the interaction
//! system never has to add or remove an outline sprite.

use bevy::asset::{AssetPath, embedded_asset, embedded_path};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d};

/// Default colour of the interaction outline: a warm gold.
pub const OUTLINE_COLOR: [f32; 4] = [1.0, 0.83, 0.29, 1.0];

/// Default outline reach past the silhouette, in texels of a 64px texture.
pub const OUTLINE_WIDTH: f32 = 2.5;

/// The whole-texture UV rectangle: top-left origin, unit size. Sprites drawn
/// from a full texture (mounds, props) use this; a sprite-sheet frame overrides
/// it with its own [`Material2d`] sub-rectangle.
pub const FULL_UV_RECT: Vec4 = Vec4::new(0.0, 0.0, 1.0, 1.0);

/// The uniform block shared with `sprite_outline.wgsl`.
///
/// The three trailing `vec4`s pad the block to a multiple of 16 bytes, which a
/// WGSL `uniform` requires.
#[derive(ShaderType, Debug, Clone, Copy, PartialEq)]
pub struct SpriteOutlineParams {
    /// `0.0` bare, `1.0` fully outlined.
    pub highlight: f32,
    /// Seconds since start, driving the outline's gentle pulse.
    pub time: f32,
    /// One texel in UV space (`1.0 / texture_size`).
    pub texel: f32,
    /// Outline reach past the silhouette, in texels.
    pub outline_width: f32,
    /// RGBA colour of the outline.
    pub color: Vec4,
    /// The sub-rectangle of the texture this quad samples: `xy` is the top-left
    /// uv and `zw` its size in uv. [`FULL_UV_RECT`] draws the whole texture; a
    /// sprite-sheet frame sets its own cell so the outline follows that frame.
    pub uv_rect: Vec4,
    /// RGBA multiplier applied to the sampled texture. White leaves the art
    /// untouched; a crop tints this while its soil is soaked.
    pub tint: Vec4,
}

impl SpriteOutlineParams {
    /// Params for a sprite drawn from a `texture_size`-pixel square texture.
    pub fn new(texture_size: f32) -> Self {
        Self {
            highlight: 0.0,
            time: 0.0,
            texel: 1.0 / texture_size.max(1.0),
            outline_width: OUTLINE_WIDTH,
            color: Vec4::from_array(OUTLINE_COLOR),
            uv_rect: FULL_UV_RECT,
            tint: Vec4::ONE,
        }
    }

    /// Params for one `frame` cell of a `grid`-wide sprite sheet whose full
    /// texture is `texture_size` pixels square. `texel` is taken from the whole
    /// sheet so the outline thickness reads the same in world space no matter how
    /// many frames the sheet holds.
    pub fn atlas(texture_size: f32, grid: u32, frame: usize) -> Self {
        let mut params = Self::new(texture_size);
        params.uv_rect = atlas_rect(grid, frame);
        params
    }
}

/// The uv rectangle of `frame` in a `grid`-by-`grid` sprite sheet, as
/// `(x, y, w, h)` with the origin at the sheet's top-left.
pub fn atlas_rect(grid: u32, frame: usize) -> Vec4 {
    let grid = grid.max(1);
    let cell = 1.0 / grid as f32;
    let column = (frame % grid as usize) as f32;
    let row = (frame / grid as usize) as f32;
    Vec4::new(column * cell, row * cell, cell, cell)
}

impl Default for SpriteOutlineParams {
    fn default() -> Self {
        Self::new(64.0)
    }
}

/// A textured quad with a togglable outline.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SpriteOutlineMaterial {
    #[uniform(0)]
    pub params: SpriteOutlineParams,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Handle<Image>,
}

impl SpriteOutlineMaterial {
    pub fn new(texture: Handle<Image>, params: SpriteOutlineParams) -> Self {
        Self { params, texture }
    }
}

impl Material2d for SpriteOutlineMaterial {
    fn fragment_shader() -> ShaderRef {
        let path = embedded_path!("sprite_outline.wgsl");
        ShaderRef::Path(AssetPath::from_path_buf(path).with_source("embedded"))
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Embeds the WGSL into the binary, keeping it out of the gitignored `assets/`
/// and out of the web build's asset requests.
pub fn register(app: &mut App) {
    embedded_asset!(app, "sprite_outline.wgsl");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_uniform_block_is_a_multiple_of_sixteen_bytes() {
        // WGSL uniform buffers must be 16-byte aligned; the trailing vec4s bring
        // the block up to size, so guard against a field being dropped.
        assert_eq!(core::mem::size_of::<SpriteOutlineParams>(), 64);
    }

    #[test]
    fn params_convert_the_texture_size_into_a_texel_step() {
        let params = SpriteOutlineParams::new(64.0);
        assert_eq!(params.texel, 1.0 / 64.0);
        assert_eq!(params.highlight, 0.0);
        assert_eq!(params.outline_width, OUTLINE_WIDTH);
        assert_eq!(params.color, Vec4::from_array(OUTLINE_COLOR));
        assert_eq!(params.uv_rect, FULL_UV_RECT);
        assert_eq!(params.tint, Vec4::ONE, "a fresh sprite is untinted");
    }

    #[test]
    fn a_full_texture_uses_the_whole_uv_rect() {
        let params = SpriteOutlineParams::new(256.0);
        assert_eq!(params.uv_rect, Vec4::new(0.0, 0.0, 1.0, 1.0));
    }

    #[test]
    fn an_atlas_frame_maps_to_its_cell() {
        // Middle cell of a 5x5 grid (frame 12): column 2, row 2 of 5.
        let rect = atlas_rect(5, 12);
        assert!((rect.x - 0.4).abs() < 1e-6);
        assert!((rect.y - 0.4).abs() < 1e-6);
        assert!((rect.z - 0.2).abs() < 1e-6);
        assert!((rect.w - 0.2).abs() < 1e-6);
    }

    #[test]
    fn the_first_atlas_frame_starts_at_the_origin() {
        assert_eq!(atlas_rect(5, 0), Vec4::new(0.0, 0.0, 0.2, 0.2));
    }

    #[test]
    fn atlas_params_keep_the_sheet_texel_but_crop_the_rect() {
        let params = SpriteOutlineParams::atlas(1280.0, 5, 5);
        assert_eq!(params.texel, 1.0 / 1280.0, "texel spans the whole sheet");
        // Frame 5 is the first cell of the second row.
        assert_eq!(params.uv_rect, Vec4::new(0.0, 0.2, 0.2, 0.2));
    }

    #[test]
    fn a_zero_texture_size_does_not_divide_by_zero() {
        assert!(SpriteOutlineParams::new(0.0).texel.is_finite());
    }
}
