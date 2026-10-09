//! Gives the farm props (crafting station, boss door) their concept art and the
//! shared interaction outline.
//!
//! Props spawn with only their gameplay components so the level loader stays
//! cheap; this plugin attaches the drawn sprite once the renderer's asset
//! resources exist, exactly like the pot mounds do. Each prop owns its own
//! material instance so its outline can light up on its own.

use crate::materials::sprite_outline::{SpriteOutlineMaterial, SpriteOutlineParams};
use crate::resources::level::LevelSet;
use bevy::prelude::*;
use bevy::sprite_render::MeshMaterial2d;

/// Side of the square art texture the prop sprites are baked down to.
pub const PROP_TEX_SIZE: u32 = 256;
/// Drawn edge length of a prop sprite, in world units.
pub const PROP_DRAW_SIZE: f32 = 96.0;
/// Outline reach for props, in texels of the 256px art. Tuned so the ring reads
/// at the same world thickness as the 64px mound default (`OUTLINE_WIDTH`).
pub const PROP_OUTLINE_WIDTH: f32 = 5.0;

/// Which concept art backs a prop's visual.
#[derive(Component, Reflect, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PropArt {
    CraftingStation,
    BossDoor,
}

impl PropArt {
    /// The asset path of this prop's baked sprite.
    pub fn texture_path(self) -> &'static str {
        match self {
            PropArt::CraftingStation => "images/props/crafting_station.png",
            PropArt::BossDoor => "images/props/boss_door.png",
        }
    }
}

/// Texture and mesh handles shared by every prop of each kind.
#[derive(Resource, Default)]
pub struct PropAssets {
    station: Option<Handle<Image>>,
    door: Option<Handle<Image>>,
    quad: Option<Handle<Mesh>>,
}

impl PropAssets {
    fn texture(&mut self, art: PropArt, server: &AssetServer) -> Handle<Image> {
        let slot = match art {
            PropArt::CraftingStation => &mut self.station,
            PropArt::BossDoor => &mut self.door,
        };
        slot.get_or_insert_with(|| server.load(art.texture_path()))
            .clone()
    }

    fn quad(&mut self, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
        self.quad
            .get_or_insert_with(|| meshes.add(Rectangle::new(PROP_DRAW_SIZE, PROP_DRAW_SIZE)))
            .clone()
    }
}

pub struct PropPlugin;

impl Plugin for PropPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PropAssets>()
            .add_systems(Update, attach_prop_visuals.after(LevelSet::Load));
    }
}

/// Adds the sprite quad and outline material to any prop that lacks one. Needs
/// the renderer's asset resources, so it does nothing in headless tests.
fn attach_prop_visuals(
    mut commands: Commands,
    server: Option<Res<AssetServer>>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<SpriteOutlineMaterial>>>,
    mut assets: ResMut<PropAssets>,
    props: Query<(Entity, &PropArt), Without<Mesh2d>>,
) {
    if props.is_empty() {
        return;
    }
    let (Some(server), Some(mut meshes), Some(mut materials)) = (server, meshes, materials) else {
        return;
    };

    let quad = assets.quad(&mut meshes);
    let mut attached = 0usize;
    for (entity, art) in props.iter() {
        let texture = assets.texture(*art, &server);
        let mut params = SpriteOutlineParams::new(PROP_TEX_SIZE as f32);
        params.outline_width = PROP_OUTLINE_WIDTH;
        let material = materials.add(SpriteOutlineMaterial::new(texture, params));
        commands
            .entity(entity)
            .insert((Mesh2d(quad.clone()), MeshMaterial2d(material)));
        attached += 1;
    }
    if attached > 0 {
        log::info!("attached prop visuals to {attached} props");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_prop_kind_has_its_own_baked_sprite() {
        let station = PropArt::CraftingStation.texture_path();
        let door = PropArt::BossDoor.texture_path();
        assert_ne!(station, door);
        assert!(station.starts_with("images/props/"));
        assert!(door.starts_with("images/props/"));
    }

    #[test]
    fn the_prop_outline_is_thicker_than_the_mound_default() {
        // The prop art is 4x the mound's texture, so the texel width has to grow
        // with it or the ring would vanish.
        const { assert!(PROP_OUTLINE_WIDTH > crate::materials::sprite_outline::OUTLINE_WIDTH) };
    }
}
