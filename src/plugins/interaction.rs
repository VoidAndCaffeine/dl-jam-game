use bevy::prelude::*;
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::state::state::State;
use crate::events::{InteractionEvent, InteractionType};
use crate::states::{DayPhase, GameState};

#[derive(Component, Reflect, Default, Debug)]
pub struct Interactable {
    pub interaction_range: f32,
}

impl Interactable {
    pub fn new(range: f32) -> Self {
        Self { interaction_range: range }
    }
}

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<InteractionEvent>()
            .add_systems(FixedUpdate, mouse_raycast_interaction)
            .add_systems(FixedUpdate, player_proximity_interaction);
    }
}

fn mouse_raycast_interaction(
    mut events: MessageWriter<InteractionEvent>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    interactables: Query<(Entity, &GlobalTransform), With<Interactable>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    game_state: Res<State<GameState>>,
    day_phase: Res<State<DayPhase>>,
) {
    if !matches!(game_state.get(), GameState::Playing) || !matches!(day_phase.get(), DayPhase::Farming) {
        return;
    }
    if !mouse_input.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = windows.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };

    let Ok((camera, camera_transform)) = cameras.single() else { return };
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) else { return };

    let mut closest_entity = None;
    let mut closest_distance = f32::INFINITY;

    for (entity, transform) in interactables.iter() {
        let distance = ray.origin.distance(transform.translation());
        if distance < closest_distance {
            closest_distance = distance;
            closest_entity = Some(entity);
        }
    }

    if let Some(entity) = closest_entity {
        events.write(InteractionEvent {
            entity,
            interaction_type: InteractionType::MouseClick,
        });
    }
}

fn player_proximity_interaction(
    mut events: MessageWriter<InteractionEvent>,
    player_query: Query<&GlobalTransform, With<crate::components::player::Player>>,
    interactables: Query<(Entity, &GlobalTransform, &Interactable)>,
    keys: Res<ButtonInput<KeyCode>>,
    game_state: Res<State<GameState>>,
    day_phase: Res<State<DayPhase>>,
) {
    if !matches!(game_state.get(), GameState::Playing) || !matches!(day_phase.get(), DayPhase::Farming) {
        return;
    }
    if !keys.just_pressed(KeyCode::Space) {
        return;
    }

    let Ok(player_transform) = player_query.single() else { return };
    let player_pos = player_transform.translation().truncate();

    let mut closest_entity = None;
    let mut closest_distance = f32::INFINITY;

    for (entity, transform, interactable) in interactables.iter() {
        let distance = player_pos.distance(transform.translation().truncate());
        if distance <= interactable.interaction_range && distance < closest_distance {
            closest_distance = distance;
            closest_entity = Some(entity);
        }
    }

    if let Some(entity) = closest_entity {
        events.write(InteractionEvent {
            entity,
            interaction_type: InteractionType::PlayerAction,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    #[test]
    fn interactable_creation() {
        let interactable = Interactable::new(60.0);
        assert_eq!(interactable.interaction_range, 60.0);
    }

    #[test]
    fn interactable_default() {
        let interactable = Interactable::default();
        assert_eq!(interactable.interaction_range, 0.0);
    }
}