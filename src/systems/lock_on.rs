use crate::components::player::Player;
use crate::components::targetable::Targetable;
use crate::constants::{
    RETICLE_SIZE, RETICLE_Z, TARGET_ARROW_DISTANCE, TARGET_ARROW_SIZE, TARGET_ARROW_Z,
};
use crate::resources::lock_on::LockOn;
use crate::states::Phase;
use crate::utils::targeting::cycle_target;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

/// A ring drawn around the locked target.
#[derive(Component)]
pub struct LockOnReticle;

/// The off-screen pointer, drawn near the player when the target is out of view.
#[derive(Component)]
pub struct LockOnArrow;

/// Cycles the lock-on target with Z (next), X (previous) or the mouse wheel.
pub fn cycle_lock_on(
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    mut lock: ResMut<LockOn>,
    targets: Query<(Entity, &Transform), With<Targetable>>,
    player: Query<&Transform, With<Player>>,
    phase: Phase,
) {
    if !phase.is_boss_fight() {
        lock.clear();
        return;
    }

    let Ok(player_transform) = player.single() else {
        return;
    };
    let origin = player_transform.translation.truncate();
    let candidates: Vec<(Entity, Vec2)> = targets
        .iter()
        .map(|(entity, transform)| (entity, transform.translation.truncate()))
        .collect();

    // Forget a target that has died or despawned.
    if let Some(current) = lock.target
        && !candidates.iter().any(|(entity, _)| *entity == current)
    {
        lock.clear();
    }

    let mut forward = keys.just_pressed(KeyCode::KeyZ);
    let mut backward = keys.just_pressed(KeyCode::KeyX);
    for event in wheel.read() {
        if event.y > 0.0 {
            forward = true;
        } else if event.y < 0.0 {
            backward = true;
        }
    }
    if !forward && !backward {
        return;
    }

    let step_forward = forward || !backward;
    if let Some(next) = cycle_target(origin, &candidates, lock.target, step_forward) {
        lock.set(next);
    }
}

/// Draws the reticle on the locked target and an arrow when it is off screen.
#[allow(clippy::too_many_arguments)]
pub fn update_lock_on_visuals(
    mut commands: Commands,
    lock: Res<LockOn>,
    player: Query<&Transform, With<Player>>,
    targets: Query<&Transform, (With<Targetable>, Without<Player>)>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    windows: Query<&Window>,
    reticles: Query<Entity, With<LockOnReticle>>,
    arrows: Query<Entity, With<LockOnArrow>>,
) {
    for entity in reticles.iter().chain(arrows.iter()) {
        commands.entity(entity).despawn();
    }

    let Some(target_entity) = lock.target else {
        return;
    };
    let Ok(target_transform) = targets.get(target_entity) else {
        return;
    };
    let target_pos = target_transform.translation.truncate();

    commands.spawn((
        LockOnReticle,
        Sprite {
            color: Color::srgba(0.6, 0.95, 1.0, 0.85),
            custom_size: Some(Vec2::splat(RETICLE_SIZE)),
            ..default()
        },
        Transform::from_xyz(target_pos.x, target_pos.y, RETICLE_Z),
        Name::new("Lock-On Reticle"),
    ));

    let Ok(player_transform) = player.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();

    // Only show the pointer when the target is outside the camera view.
    let off_screen = match (cameras.single(), windows.single()) {
        (Ok((camera, camera_transform)), Ok(window)) => {
            match camera.world_to_viewport(camera_transform, target_pos.extend(0.0)) {
                Ok(viewport) => {
                    let size = window.resolution.size();
                    viewport.x < 0.0
                        || viewport.y < 0.0
                        || viewport.x > size.x
                        || viewport.y > size.y
                }
                Err(_) => true,
            }
        }
        // No camera (headless): treat it as off screen so the arrow still exists.
        _ => true,
    };
    if !off_screen {
        return;
    }

    let direction = (target_pos - player_pos).normalize_or(Vec2::X);
    let arrow_pos = player_pos + direction * TARGET_ARROW_DISTANCE;
    commands.spawn((
        LockOnArrow,
        Sprite {
            color: Color::srgba(0.6, 0.95, 1.0, 0.9),
            custom_size: Some(Vec2::splat(TARGET_ARROW_SIZE)),
            ..default()
        },
        Transform::from_xyz(arrow_pos.x, arrow_pos.y, TARGET_ARROW_Z)
            .with_rotation(Quat::from_rotation_z(direction.y.atan2(direction.x))),
        Name::new("Lock-On Arrow"),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::states::{DayPhase, GameState};
    use bevy::state::app::StatesPlugin;
    use bevy::transform::TransformPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<LockOn>()
            .add_message::<MouseWheel>()
            .add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_systems(Update, cycle_lock_on);
        app.world_mut()
            .spawn((Player, Transform::from_xyz(0.0, 0.0, 0.0)));
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossFight);
        app.update();
        app
    }

    fn spawn_target(app: &mut App, pos: Vec2) -> Entity {
        app.world_mut()
            .spawn((Targetable, Transform::from_xyz(pos.x, pos.y, 0.0)))
            .id()
    }

    #[test]
    fn z_locks_the_nearest_target() {
        let mut app = setup_app();
        let near = spawn_target(&mut app, Vec2::new(30.0, 0.0));
        spawn_target(&mut app, Vec2::new(300.0, 0.0));

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyZ);
        app.update();

        assert_eq!(app.world().resource::<LockOn>().target, Some(near));
    }

    #[test]
    fn z_cycles_and_wraps_between_targets() {
        let mut app = setup_app();
        let a = spawn_target(&mut app, Vec2::new(30.0, 0.0));
        let b = spawn_target(&mut app, Vec2::new(60.0, 0.0));

        let tap = |app: &mut App, key| {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(key);
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .press(key);
            app.update();
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .clear_just_pressed(key);
        };

        tap(&mut app, KeyCode::KeyZ);
        assert_eq!(app.world().resource::<LockOn>().target, Some(a));
        tap(&mut app, KeyCode::KeyZ);
        assert_eq!(app.world().resource::<LockOn>().target, Some(b));
        tap(&mut app, KeyCode::KeyZ);
        assert_eq!(app.world().resource::<LockOn>().target, Some(a));
    }

    #[test]
    fn a_dead_target_is_forgotten() {
        let mut app = setup_app();
        let target = spawn_target(&mut app, Vec2::new(30.0, 0.0));
        app.world_mut().resource_mut::<LockOn>().set(target);

        app.world_mut().entity_mut(target).despawn();
        app.update();

        assert!(app.world().resource::<LockOn>().target.is_none());
    }

    #[test]
    fn leaving_the_fight_clears_the_lock() {
        let mut app = setup_app();
        let target = spawn_target(&mut app, Vec2::new(30.0, 0.0));
        app.world_mut().resource_mut::<LockOn>().set(target);

        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);
        app.update();

        assert!(app.world().resource::<LockOn>().target.is_none());
    }
}
