//! Procedural swing graphics: a shader material that draws the player's light
//! (poke) and heavy (slash) attacks without any baked sprite sheet.

use bevy::asset::{AssetPath, embedded_asset, embedded_path};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d};

/// Shader code selecting the thrusting poke.
pub const ATTACK_POKE: u32 = 0;
/// Shader code selecting the sweeping slash.
pub const ATTACK_SLASH: u32 = 1;

/// The uniform block shared with `attack_effect.wgsl`.
///
/// The trailing pad fields keep the block a multiple of 16 bytes, which a WGSL
/// `uniform` requires.
#[derive(ShaderType, Debug, Clone, Copy, PartialEq, Default)]
pub struct AttackEffectParams {
    /// `ATTACK_POKE` or `ATTACK_SLASH`.
    pub attack_type: u32,
    /// `GearSet` index, selecting the colour theme.
    pub theme: u32,
    /// `0.0..=1.0` across the swing's lifetime.
    pub progress: f32,
    /// World reach of the swing.
    pub reach: f32,
    /// Base width of a poke, in world units.
    pub width: f32,
    /// Seconds since the swing began.
    pub time: f32,
    pad0: f32,
    pad1: f32,
}

impl AttackEffectParams {
    pub fn new(attack_type: u32, theme: u32, reach: f32, width: f32) -> Self {
        Self {
            attack_type,
            theme,
            progress: 0.0,
            reach,
            width,
            time: 0.0,
            pad0: 0.0,
            pad1: 0.0,
        }
    }
}

/// Draws one swing entirely in the shader. A fresh instance lives per swing so
/// its `progress` can be animated until the swing despawns.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct AttackEffectMaterial {
    #[uniform(0)]
    pub params: AttackEffectParams,
}

impl AttackEffectMaterial {
    pub fn new(params: AttackEffectParams) -> Self {
        Self { params }
    }
}

impl Material2d for AttackEffectMaterial {
    fn fragment_shader() -> ShaderRef {
        let path = embedded_path!("attack_effect.wgsl");
        ShaderRef::Path(AssetPath::from_path_buf(path).with_source("embedded"))
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Embeds the WGSL into the binary.
///
/// The shader is source rather than generated art, so it lives beside this file
/// instead of in the gitignored `assets/`. Embedding also keeps it out of the
/// web build's asset requests.
pub fn register(app: &mut App) {
    embedded_asset!(app, "attack_effect.wgsl");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_uniform_block_is_a_multiple_of_sixteen_bytes() {
        // WGSL uniform buffers must be 16-byte aligned; the padding fields are
        // there to satisfy that, so guard against a future field being dropped.
        assert_eq!(core::mem::size_of::<AttackEffectParams>(), 32);
    }

    #[test]
    fn a_fresh_material_starts_at_zero_progress() {
        let material =
            AttackEffectMaterial::new(AttackEffectParams::new(ATTACK_POKE, 2, 70.0, 28.0));
        assert_eq!(material.params.progress, 0.0);
        assert_eq!(material.params.time, 0.0);
        assert_eq!(material.params.attack_type, ATTACK_POKE);
        assert_eq!(material.params.theme, 2);
    }
}
