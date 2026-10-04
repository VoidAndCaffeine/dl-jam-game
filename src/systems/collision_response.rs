use crate::components::collider::{Collider, CollisionEvent};
use crate::components::player::Player;
use bevy::prelude::*;

pub fn collision_response(
    trigger: On<CollisionEvent>,
    mut player_query: Query<(&mut Transform, &Collider), With<Player>>,
    static_query: Query<(&Transform, &Collider), (With<Collider>, Without<Player>)>,
) {
    let event = trigger.event();
    let is_a_player = player_query.contains(event.entity_a);
    let is_b_player = player_query.contains(event.entity_b);

    if !is_a_player && !is_b_player {
        return;
    }

    let (player_entity, static_entity) = if is_a_player {
        (event.entity_a, event.entity_b)
    } else {
        (event.entity_b, event.entity_a)
    };

    let Ok((mut player_transform, player_collider)) = player_query.get_mut(player_entity) else {
        return;
    };
    let Ok((static_transform, static_collider)) = static_query.get(static_entity) else {
        return;
    };

    let player_half = player_collider.size * 0.5;
    let static_half = static_collider.size * 0.5;

    let player_min = player_transform.translation.truncate() - player_half;
    let player_max = player_transform.translation.truncate() + player_half;
    let static_min = static_transform.translation.truncate() - static_half;
    let static_max = static_transform.translation.truncate() + static_half;

    let overlap_x = (player_max.x - static_min.x).min(static_max.x - player_min.x);
    let overlap_y = (player_max.y - static_min.y).min(static_max.y - player_min.y);

    if overlap_x < overlap_y {
        let player_center = player_transform.translation.truncate();
        let static_center = static_transform.translation.truncate();
        let direction = if player_center.x < static_center.x {
            -1.0
        } else {
            1.0
        };
        player_transform.translation.x =
            static_center.x + direction * (static_half.x + player_half.x);
    } else {
        let player_center = player_transform.translation.truncate();
        let static_center = static_transform.translation.truncate();
        let direction = if player_center.y < static_center.y {
            -1.0
        } else {
            1.0
        };
        player_transform.translation.y =
            static_center.y + direction * (static_half.y + player_half.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_observer(collision_response);
        app
    }

    fn spawn_player(app: &mut App, pos: Vec2, size: Vec2) -> Entity {
        app.world_mut().spawn((
            Player,
            Collider { size, is_solid: true },
            Transform::from_xyz(pos.x, pos.y, 0.0),
        )).id()
    }

    fn spawn_static(app: &mut App, pos: Vec2, size: Vec2) -> Entity {
        app.world_mut().spawn((
            Collider { size, is_solid: true },
            Transform::from_xyz(pos.x, pos.y, 0.0),
        )).id()
    }

    fn get_player_transform(app: &mut App, entity: Entity) -> Transform {
        app.world_mut().query::<&Transform>().get(app.world(), entity).unwrap().clone()
    }

    #[test]
    fn pushes_player_out_on_x_axis() {
        let mut app = setup_app();
        let player = spawn_player(&mut app, Vec2::new(10.0, 0.0), Vec2::splat(32.0));
        let static_e = spawn_static(&mut app, Vec2::ZERO, Vec2::splat(32.0));
        
        // Trigger collision from left side (player center at 10, static at 0)
        app.world_mut().trigger(CollisionEvent { entity_a: player, entity_b: static_e });
        
        let transform = get_player_transform(&mut app, player);
        // Player should be pushed to the right of static
        assert!(transform.translation.x > 16.0); // static_half.x (16) + player_half.x (16) = 32
    }

    #[test]
    fn pushes_player_out_on_y_axis() {
        let mut app = setup_app();
        let player = spawn_player(&mut app, Vec2::new(0.0, 10.0), Vec2::splat(32.0));
        let static_e = spawn_static(&mut app, Vec2::ZERO, Vec2::splat(32.0));
        
        // Trigger collision from top (player center at y=10, static at y=0)
        app.world_mut().trigger(CollisionEvent { entity_a: player, entity_b: static_e });
        
        let transform = get_player_transform(&mut app, player);
        // Player should be pushed above static
        assert!(transform.translation.y > 16.0);
    }

    #[test]
    fn chooses_minimum_penetration_axis() {
        let mut app = setup_app();
        // Player overlapping more on Y than X
        // Static at (0,0), player at (5, 20) with 32x32 size
        // X overlap: (16 - (-11)) = 27, Y overlap: (16 - 4) = 12
        // Min is Y, so should push on Y
        let player = spawn_player(&mut app, Vec2::new(5.0, 20.0), Vec2::splat(32.0));
        let static_e = spawn_static(&mut app, Vec2::ZERO, Vec2::splat(32.0));
        
        app.world_mut().trigger(CollisionEvent { entity_a: player, entity_b: static_e });
        
        let transform = get_player_transform(&mut app, player);
        // Should push on Y (vertical), so X stays ~5, Y moves
        assert!((transform.translation.x - 5.0).abs() < 1.0);
        assert!(transform.translation.y > 16.0);
    }
}
