//! Shared procedural circles.
//!
//! A few effects want a filled circle rather than a quad sprite: the hit sparks
//! and the auto-target indicator. Rather than ship a texture for a shape this
//! simple, they draw a shared unit-circle mesh scaled to the wanted radius, with
//! a small cache of tinted materials. Everything lives behind optional resources
//! so a headless run (no renderer) simply skips the graphic.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};

/// The shared unit circle and a cache of tinted materials.
#[derive(Resource, Default)]
pub struct CircleArt {
    mesh: Option<Handle<Mesh>>,
    materials: Vec<(Color, Handle<ColorMaterial>)>,
}

impl CircleArt {
    fn mesh(&mut self, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
        self.mesh
            .get_or_insert_with(|| meshes.add(Circle::new(1.0)))
            .clone()
    }

    fn material(
        &mut self,
        materials: &mut Assets<ColorMaterial>,
        color: Color,
    ) -> Handle<ColorMaterial> {
        if let Some((_, handle)) = self.materials.iter().find(|(cached, _)| *cached == color) {
            return handle.clone();
        }
        let handle = materials.add(color);
        self.materials.push((color, handle.clone()));
        handle
    }
}

/// The renderer-dependent handles a circle needs, all optional so a headless run
/// skips the graphic instead of panicking on a missing `Assets<_>`.
#[derive(SystemParam)]
pub struct CircleAccess<'w> {
    meshes: Option<ResMut<'w, Assets<Mesh>>>,
    materials: Option<ResMut<'w, Assets<ColorMaterial>>>,
    art: Option<ResMut<'w, CircleArt>>,
}

impl CircleAccess<'_> {
    /// A `Mesh2d`/`MeshMaterial2d` pair for a filled circle tinted `color`, or
    /// `None` without a renderer. The mesh is a unit circle, so the caller scales
    /// the entity's transform to the wanted radius.
    pub fn circle(&mut self, color: Color) -> Option<(Mesh2d, MeshMaterial2d<ColorMaterial>)> {
        let meshes = self.meshes.as_deref_mut()?;
        let materials = self.materials.as_deref_mut()?;
        let art = self.art.as_deref_mut()?;
        let mesh = art.mesh(meshes);
        let material = art.material(materials, color);
        Some((Mesh2d(mesh), MeshMaterial2d(material)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shared_mesh_is_built_once() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>();
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        let mut art = CircleArt::default();
        let first = art.mesh(&mut meshes);
        let second = art.mesh(&mut meshes);
        assert_eq!(first, second);
    }

    #[test]
    fn tinted_materials_are_cached_per_colour() {
        let mut app = App::new();
        app.init_resource::<Assets<ColorMaterial>>();
        let mut materials = app.world_mut().resource_mut::<Assets<ColorMaterial>>();
        let mut art = CircleArt::default();
        let red = Color::srgb(1.0, 0.0, 0.0);
        let blue = Color::srgb(0.0, 0.0, 1.0);
        let first_red = art.material(&mut materials, red);
        let second_red = art.material(&mut materials, red);
        let blue_handle = art.material(&mut materials, blue);
        assert_eq!(first_red, second_red, "the same colour reuses its material");
        assert_ne!(first_red, blue_handle);
    }
}
