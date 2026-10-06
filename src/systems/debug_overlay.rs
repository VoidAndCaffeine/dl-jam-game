use crate::components::attack::AttackVisual;
use crate::components::boss::{BOSS_SIZE, Boss};
use crate::components::collider::Collider;
use crate::components::player::Player;
use crate::constants::BOSS_HURTBOX_SCALE;
use crate::resources::debug_overlay::DebugOverlay;
use bevy::gizmos::prelude::*;
use bevy::prelude::*;

/// F3 flips the collision-box debug overlay.
pub fn toggle_debug_overlay(keys: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<DebugOverlay>) {
    if keys.just_pressed(KeyCode::F3) {
        overlay.show_aabbs = !overlay.show_aabbs;
    }
}

/// Draws the player, boss hurtbox and active swing boxes.
///
/// Only run where a renderer exists; the caller gates registration on the gizmo
/// config resource so headless tests are unaffected.
pub fn debug_aabb_overlay(
    overlay: Res<DebugOverlay>,
    mut gizmos: Gizmos,
    player: Query<(&Transform, &Collider), With<Player>>,
    bosses: Query<&Transform, With<Boss>>,
    swings: Query<(&Transform, &Sprite), With<AttackVisual>>,
) {
    if !overlay.show_aabbs {
        return;
    }

    for (transform, collider) in player.iter() {
        gizmos.rect_2d(
            isometry(transform),
            collider.size,
            Color::srgb(0.2, 0.8, 1.0),
        );
    }

    for transform in bosses.iter() {
        gizmos.rect_2d(
            isometry(transform),
            Vec2::splat(BOSS_SIZE * BOSS_HURTBOX_SCALE),
            Color::srgb(0.2, 1.0, 0.3),
        );
    }

    for (transform, sprite) in swings.iter() {
        let Some(size) = sprite.custom_size else {
            continue;
        };
        gizmos.rect_2d(isometry(transform), size, Color::srgb(1.0, 0.4, 1.0));
    }
}

/// Translates a 2D transform's position and z-rotation into an isometry.
fn isometry(transform: &Transform) -> Isometry2d {
    let angle = transform.rotation.to_scaled_axis().z;
    Isometry2d::new(transform.translation.truncate(), Rot2::radians(angle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f3_toggles_the_overlay() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<DebugOverlay>()
            .add_systems(Update, toggle_debug_overlay);
        app.update();
        assert!(!app.world().resource::<DebugOverlay>().show_aabbs);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F3);
        app.update();
        assert!(app.world().resource::<DebugOverlay>().show_aabbs);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear_just_pressed(KeyCode::F3);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(KeyCode::F3);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::F3);
        app.update();
        assert!(!app.world().resource::<DebugOverlay>().show_aabbs);
    }
}
