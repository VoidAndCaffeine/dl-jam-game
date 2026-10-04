use bevy::prelude::*;

#[derive(Message, Debug, Clone)]
pub struct InteractionEvent {
    pub entity: Entity,
    pub interaction_type: InteractionType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionType {
    FarmAction,
    Crafting,
    BossArena,
    NPC,
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

#[derive(Message, Debug, Clone, Copy)]
pub struct GearCrafted(pub crate::components::gear::GearPiece);

#[derive(Message, Debug, Clone, Copy)]
pub struct GearEquipped(pub crate::components::gear::GearSlot);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interaction_event_creation() {
        let mut app = App::new();
        let entity = app.world_mut().spawn_empty().id();
        let event = InteractionEvent {
            entity,
            interaction_type: InteractionType::FarmAction,
        };
        assert_eq!(event.entity, entity);
        assert_eq!(event.interaction_type, InteractionType::FarmAction);
    }

    #[test]
    fn day_advanced_event_creation() {
        let event = DayAdvanced { day: 5 };
        assert_eq!(event.day, 5);
    }

    #[test]
    fn gear_messages_creation() {
        let piece = crate::components::gear::GearPiece::new(
            crate::components::gear::GearSet::Starter,
            crate::components::gear::GearSlot::Weapon,
        );
        let crafted = GearCrafted(piece);
        assert_eq!(crafted.0, piece);
        assert_eq!(
            GearEquipped(piece.slot).0,
            crate::components::gear::GearSlot::Weapon
        );
    }
}
