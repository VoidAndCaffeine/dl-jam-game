//! Spawns and animates the procedural swing graphics.
//!
//! Attack graphics are drawn by a shader rather than a baked sprite sheet, so
//! this module owns the tiny bit of plumbing that needs: a shared unit quad, a
//! material instance per swing, and the per-frame `progress` write that animates
//! it. All three live behind optional resources, so headless tests (which run
//! without an asset server) still spawn the gameplay-side [`AttackVisual`] and
//! simply skip the mesh.

use crate::components::attack::{AttackType, AttackVisual};
use crate::components::gear::GearSet;
use crate::materials::attack_effect::{
    ATTACK_POKE, ATTACK_SLASH, AttackEffectMaterial, AttackEffectParams,
};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::sprite_render::MeshMaterial2d;

/// Swing graphics sit in front of every world sprite.
pub const SWING_Z: f32 = 2.0;

/// The shared quad every swing is drawn on, made once and kept.
#[derive(Resource, Default)]
pub struct AttackEffectAssets {
    quad: Option<Handle<Mesh>>,
}

impl AttackEffectAssets {
    fn quad(&mut self, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
        self.quad
            .get_or_insert_with(|| meshes.add(Rectangle::new(1.0, 1.0)))
            .clone()
    }
}

/// The asset handles a swing needs, every one optional so a headless run skips
/// the graphic instead of panicking on a missing `Assets<_>`.
#[derive(SystemParam)]
pub struct AttackEffectAccess<'w> {
    meshes: Option<ResMut<'w, Assets<Mesh>>>,
    materials: Option<ResMut<'w, Assets<AttackEffectMaterial>>>,
    assets: Option<ResMut<'w, AttackEffectAssets>>,
}

impl AttackEffectAccess<'_> {
    /// The shared unit quad, or `None` without a renderer.
    fn quad(&mut self) -> Option<Handle<Mesh>> {
        let meshes = self.meshes.as_deref_mut()?;
        let assets = self.assets.as_deref_mut()?;
        Some(assets.quad(meshes))
    }

    /// Registers a fresh material instance, or `None` without a renderer.
    fn add(&mut self, params: AttackEffectParams) -> Option<Handle<AttackEffectMaterial>> {
        let materials = self.materials.as_deref_mut()?;
        Some(materials.add(AttackEffectMaterial::new(params)))
    }

    /// Advances a live swing's uniforms. A no-op for a missing renderer.
    pub fn set_progress(
        &mut self,
        handle: &Handle<AttackEffectMaterial>,
        progress: f32,
        time: f32,
    ) {
        let Some(materials) = self.materials.as_deref_mut() else {
            return;
        };
        if let Some(mut material) = materials.get_mut(handle) {
            material.params.progress = progress;
            material.params.time = time;
        }
    }
}

/// Spawns one swing at `origin`, facing `facing`.
///
/// The [`AttackVisual`] is always attached so the debug overlay can draw the
/// hitbox; the shader-driven mesh is only attached when a renderer is present.
/// `set` picks the colour theme, matching the equipped weapon.
#[allow(clippy::too_many_arguments)]
pub fn spawn_swing(
    commands: &mut Commands,
    effects: &mut AttackEffectAccess,
    origin: Vec2,
    facing: Vec2,
    reach: f32,
    width: f32,
    attack: AttackType,
    set: Option<GearSet>,
) {
    let facing = if facing == Vec2::ZERO {
        Vec2::X
    } else {
        facing.normalize()
    };
    let center = origin + facing * (reach * 0.5);
    let angle = facing.y.atan2(facing.x);
    let transform = Transform::from_translation(center.extend(SWING_Z))
        .with_rotation(Quat::from_rotation_z(angle))
        .with_scale(Vec3::new(reach, 2.0 * reach, 1.0));

    let mut entity = commands.spawn((
        AttackVisual::new(attack, Vec2::new(reach, width)),
        transform,
        Name::new(format!("{} Swing", attack.label())),
    ));

    let kind = match attack {
        AttackType::Light => ATTACK_POKE,
        AttackType::Heavy => ATTACK_SLASH,
    };
    let theme = set.map_or(0, |set| set.index() as u32);
    if let (Some(mesh), Some(material)) = (
        effects.quad(),
        effects.add(AttackEffectParams::new(kind, theme, reach, width)),
    ) {
        entity.insert((Mesh2d(mesh), MeshMaterial2d(material)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::transform::TransformPlugin;

    /// Drives `spawn_swing` through a real system so the `SystemParam` resolves
    /// exactly the way it does in-game.
    fn spawn_test_swing(mut commands: Commands, mut effects: AttackEffectAccess) {
        spawn_swing(
            &mut commands,
            &mut effects,
            Vec2::new(10.0, 20.0),
            Vec2::X,
            70.0,
            28.0,
            AttackType::Heavy,
            Some(GearSet::Master),
        );
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin))
            .init_resource::<AttackEffectAssets>()
            .add_systems(Update, spawn_test_swing);
        app.update();
        app
    }

    #[test]
    fn a_swing_still_spawns_its_visual_without_a_renderer() {
        let mut app = setup_app();
        let visual = app
            .world_mut()
            .query_filtered::<&AttackVisual, With<AttackVisual>>()
            .iter(app.world())
            .next()
            .copied()
            .expect("the swing visual is spawned");
        assert_eq!(visual.attack_type, AttackType::Heavy);
        assert_eq!(visual.hitbox, Vec2::new(70.0, 28.0));
    }

    #[test]
    fn no_mesh_is_spawned_without_an_asset_server() {
        let mut app = setup_app();
        let meshes = app
            .world_mut()
            .query_filtered::<Entity, With<Mesh2d>>()
            .iter(app.world())
            .count();
        assert_eq!(meshes, 0, "headless runs skip the shader-driven mesh");
    }

    #[test]
    fn the_shared_quad_is_built_once() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>();
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        let mut assets = AttackEffectAssets::default();
        let first = assets.quad(&mut meshes);
        let second = assets.quad(&mut meshes);
        assert_eq!(first, second);
    }
}
