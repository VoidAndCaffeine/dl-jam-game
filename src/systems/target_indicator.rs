use crate::components::player::Player;
use crate::components::targetable::Targetable;
use crate::constants::{TARGET_INDICATOR_COLOR, TARGET_INDICATOR_RADIUS, TARGET_INDICATOR_Z};
use crate::resources::circle_art::CircleAccess;
use crate::states::Phase;
use crate::utils::targeting::nearest_target;
use bevy::prelude::*;

/// A small circle drawn over the target a swing will auto-face.
#[derive(Component)]
pub struct TargetIndicator;

/// Marks the nearest targetable during a fight, so the player can see who a
/// swing is about to point at. Redrawn every frame; there is only ever one.
pub fn update_target_indicator(
    mut commands: Commands,
    phase: Phase,
    mut circles: CircleAccess,
    player: Query<&Transform, With<Player>>,
    targets: Query<(Entity, &Transform), With<Targetable>>,
    existing: Query<Entity, With<TargetIndicator>>,
) {
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }
    if !phase.is_boss_fight() {
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
    let Some(target) = nearest_target(origin, &candidates) else {
        return;
    };
    let Some(position) = candidates
        .iter()
        .find(|(entity, _)| *entity == target)
        .map(|(_, position)| *position)
    else {
        return;
    };
    let Some((mesh, material)) = circles.circle(TARGET_INDICATOR_COLOR) else {
        return;
    };
    commands.spawn((
        TargetIndicator,
        mesh,
        material,
        Transform::from_translation(position.extend(TARGET_INDICATOR_Z))
            .with_scale(Vec3::splat(TARGET_INDICATOR_RADIUS)),
        Name::new("Target Indicator"),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::circle_art::CircleArt;
    use crate::states::{DayPhase, GameState};
    use bevy::state::app::StatesPlugin;
    use bevy::transform::TransformPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<ColorMaterial>>()
            .init_resource::<CircleArt>()
            .add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_systems(Update, update_target_indicator);
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

    fn indicators(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<TargetIndicator>>()
            .iter(app.world())
            .collect()
    }

    #[test]
    fn the_indicator_sits_on_the_nearest_target() {
        let mut app = setup_app();
        spawn_target(&mut app, Vec2::new(300.0, 0.0));
        spawn_target(&mut app, Vec2::new(40.0, 0.0));
        app.update();

        let indicators = indicators(&mut app);
        assert_eq!(indicators.len(), 1);
        let transform = app.world().get::<Transform>(indicators[0]).unwrap();
        assert_eq!(transform.translation.truncate(), Vec2::new(40.0, 0.0));
        assert!(app.world().get::<Mesh2d>(indicators[0]).is_some());
    }

    #[test]
    fn the_indicator_follows_the_nearest_target_as_it_moves() {
        let mut app = setup_app();
        let target = spawn_target(&mut app, Vec2::new(40.0, 0.0));
        app.update();

        app.world_mut()
            .get_mut::<Transform>(target)
            .unwrap()
            .translation = Vec3::new(-60.0, 10.0, 0.0);
        app.update();

        let indicators = indicators(&mut app);
        assert_eq!(indicators.len(), 1, "the old indicator is replaced");
        let transform = app.world().get::<Transform>(indicators[0]).unwrap();
        assert_eq!(transform.translation.truncate(), Vec2::new(-60.0, 10.0));
    }

    #[test]
    fn there_is_no_indicator_outside_a_fight() {
        let mut app = setup_app();
        spawn_target(&mut app, Vec2::new(40.0, 0.0));
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);
        app.update();

        assert!(indicators(&mut app).is_empty());
    }
}
