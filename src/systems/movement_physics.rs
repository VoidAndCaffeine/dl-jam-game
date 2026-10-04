use crate::components::player::{Movement, Player};
use bevy::prelude::*;
use bevy::time::TimePlugin;

pub fn movement_physics(
    time: Res<Time>,
    mut player_query: Query<(&Movement, &mut Transform), With<Player>>,
) {
    for (movement, mut transform) in player_query.iter_mut() {
        let velocity = movement.input_direction * movement.speed;
        transform.translation.x += velocity.x * time.delta_secs();
        transform.translation.y += velocity.y * time.delta_secs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;
    use bevy::time::TimePlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_plugins(TimePlugin);
        app.world_mut().spawn((
            Player,
            Movement::default(),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));
        app.add_systems(Update, movement_physics); // Use Update for testing
        app
    }

    fn get_transform(app: &mut App) -> Transform {
        app.world_mut().query::<&Transform>().single(app.world_mut()).unwrap().clone()
    }

    fn set_input_direction(app: &mut App, dir: Vec2) {
        app.world_mut().query::<&mut Movement>().single_mut(app.world_mut()).unwrap().input_direction = dir;
    }

    #[test]
    fn applies_velocity_to_transform() {
        let mut app = setup_app();
        set_input_direction(&mut app, Vec2::X);
        // Advance time to get a non-zero delta
        app.world_mut().resource_mut::<Time<Real>>().advance_by(std::time::Duration::from_millis(16));
        app.update();
        let transform = get_transform(&mut app);
        assert!(transform.translation.x > 0.0);
        assert_eq!(transform.translation.y, 0.0);
    }

    #[test]
    fn respects_delta_time_scaling() {
        let mut app = setup_app();
        set_input_direction(&mut app, Vec2::X);
        
        // First update with small delta
        app.world_mut().resource_mut::<Time<Real>>().advance_by(std::time::Duration::from_millis(16));
        app.update();
        let pos_after_first = get_transform(&mut app).translation.x;
        
        // Advance time by 2 seconds and update again
        app.world_mut().resource_mut::<Time<Real>>().advance_by(std::time::Duration::from_secs(2));
        app.update();
        let pos_after_second = get_transform(&mut app).translation.x;
        
        // Should have moved in the second update
        assert!(pos_after_second > pos_after_first);
    }

    #[test]
    fn speed_multiplier_affects_velocity() {
        let mut app = setup_app();
        set_input_direction(&mut app, Vec2::X);
        app.world_mut().query::<&mut Movement>().single_mut(app.world_mut()).unwrap().speed = 400.0; // 2x default
        
        app.world_mut().resource_mut::<Time<Real>>().advance_by(std::time::Duration::from_millis(16));
        app.update();
        let pos_fast = get_transform(&mut app).translation.x;
        
        // Reset and test with default speed
        let mut app2 = setup_app();
        set_input_direction(&mut app2, Vec2::X);
        app2.world_mut().resource_mut::<Time<Real>>().advance_by(std::time::Duration::from_millis(16));
        app2.update();
        let pos_normal = get_transform(&mut app2).translation.x;
        
        assert!((pos_fast / pos_normal - 2.0).abs() < 0.1);
    }
}