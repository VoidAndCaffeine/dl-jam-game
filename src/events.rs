use bevy::prelude::*;

#[derive(Message, Debug, Clone)]
pub struct InteractionEvent {
    pub entity: Entity,
    pub interaction_type: InteractionType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionType {
    MouseClick,
    PlayerAction,
}

#[derive(Message, Debug, Clone)]
pub struct DayAdvanced {
    pub day: u32,
}

#[derive(Message, Debug, Clone)]
pub struct CropPlanted(pub crate::components::pot::CropType);

#[derive(Message, Debug, Clone)]
pub struct CropWatered;

#[derive(Message, Debug, Clone)]
pub struct CropHarvested(pub crate::components::pot::CropType);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interaction_event_creation() {
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();
        let event = InteractionEvent {
            entity,
            interaction_type: InteractionType::MouseClick,
        };
        assert_eq!(event.entity, entity);
        assert_eq!(event.interaction_type, InteractionType::MouseClick);
    }

    #[test]
    fn day_advanced_event_creation() {
        let event = DayAdvanced { day: 5 };
        assert_eq!(event.day, 5);
    }
}