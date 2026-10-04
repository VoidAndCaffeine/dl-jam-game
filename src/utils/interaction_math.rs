use crate::components::player::INTERACTION_RANGE;
use crate::events::InteractionType;
use crate::plugins::interaction::{BossArenaEntry, CraftingStation, FarmPot, Interactable, NPC};
use bevy::ecs::world::World;
use bevy::prelude::*;

pub fn find_closest_in_range(
    player_pos: Vec2,
    interactables: &[(Entity, Vec2)],
    range: f32,
) -> Option<Entity> {
    let mut closest_entity = None;
    let mut closest_distance = f32::INFINITY;

    for (entity, pos) in interactables.iter() {
        let distance = player_pos.distance(*pos);
        if distance <= range && distance < closest_distance {
            closest_distance = distance;
            closest_entity = Some(*entity);
        }
    }

    closest_entity
}

pub fn find_closest_to_ray(ray_origin: Vec2, candidates: &[(Entity, Vec2)]) -> Option<Entity> {
    let mut closest_entity = None;
    let mut closest_distance = f32::INFINITY;

    for (entity, pos) in candidates.iter() {
        let distance = ray_origin.distance(*pos);
        if distance < closest_distance {
            closest_distance = distance;
            closest_entity = Some(*entity);
        }
    }

    closest_entity
}

pub fn resolve_interaction_type(entity: Entity, world: &World) -> InteractionType {
    if world.get::<FarmPot>(entity).is_some() {
        return InteractionType::FarmAction;
    }
    if world.get::<CraftingStation>(entity).is_some() {
        return InteractionType::Crafting;
    }
    if world.get::<BossArenaEntry>(entity).is_some() {
        return InteractionType::BossArena;
    }
    if world.get::<NPC>(entity).is_some() {
        return InteractionType::NPC;
    }
    InteractionType::FarmAction
}

pub fn resolve_interaction_type_from_queries(
    entity: Entity,
    farm_pots: &Query<&FarmPot>,
    crafting_stations: &Query<&CraftingStation>,
    boss_arenas: &Query<&BossArenaEntry>,
    npcs: &Query<&NPC>,
) -> InteractionType {
    if farm_pots.get(entity).is_ok() {
        return InteractionType::FarmAction;
    }
    if crafting_stations.get(entity).is_ok() {
        return InteractionType::Crafting;
    }
    if boss_arenas.get(entity).is_ok() {
        return InteractionType::BossArena;
    }
    if npcs.get(entity).is_ok() {
        return InteractionType::NPC;
    }
    InteractionType::FarmAction
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    fn setup_app() -> App {
        App::new()
    }

    fn spawn_entity(app: &mut App, pos: Vec2) -> Entity {
        app.world_mut()
            .spawn((
                Transform::from_xyz(pos.x, pos.y, 0.0),
                GlobalTransform::default(),
            ))
            .id()
    }

    #[test]
    fn find_closest_in_range_returns_none_when_empty() {
        let mut app = setup_app();
        let player_pos = Vec2::ZERO;
        let interactables: Vec<(Entity, Vec2)> = vec![];
        let result = find_closest_in_range(player_pos, &interactables, INTERACTION_RANGE);
        assert_eq!(result, None);
    }

    #[test]
    fn find_closest_in_range_returns_none_when_all_out_of_range() {
        let mut app = setup_app();
        let player_pos = Vec2::ZERO;
        let e1 = spawn_entity(&mut app, Vec2::new(100.0, 0.0));
        let e2 = spawn_entity(&mut app, Vec2::new(0.0, 100.0));
        let interactables = vec![(e1, Vec2::new(100.0, 0.0)), (e2, Vec2::new(0.0, 100.0))];
        let result = find_closest_in_range(player_pos, &interactables, INTERACTION_RANGE);
        assert_eq!(result, None);
    }

    #[test]
    fn find_closest_in_range_picks_closest() {
        let mut app = setup_app();
        let player_pos = Vec2::ZERO;
        let e1 = spawn_entity(&mut app, Vec2::new(20.0, 0.0));
        let e2 = spawn_entity(&mut app, Vec2::new(10.0, 0.0));
        let e3 = spawn_entity(&mut app, Vec2::new(30.0, 0.0));
        let interactables = vec![
            (e1, Vec2::new(20.0, 0.0)),
            (e2, Vec2::new(10.0, 0.0)),
            (e3, Vec2::new(30.0, 0.0)),
        ];
        let result = find_closest_in_range(player_pos, &interactables, INTERACTION_RANGE);
        assert_eq!(result, Some(e2));
    }

    #[test]
    fn find_closest_in_range_excludes_out_of_range() {
        let mut app = setup_app();
        let player_pos = Vec2::ZERO;
        let e1 = spawn_entity(&mut app, Vec2::new(20.0, 0.0));
        let e2 = spawn_entity(&mut app, Vec2::new(50.0, 0.0));
        let interactables = vec![(e1, Vec2::new(20.0, 0.0)), (e2, Vec2::new(50.0, 0.0))];
        let result = find_closest_in_range(player_pos, &interactables, INTERACTION_RANGE);
        assert_eq!(result, Some(e1));
    }

    #[test]
    fn find_closest_to_ray_returns_none_when_empty() {
        let candidates: Vec<(Entity, Vec2)> = vec![];
        let result = find_closest_to_ray(Vec2::ZERO, &candidates);
        assert_eq!(result, None);
    }

    #[test]
    fn find_closest_to_ray_picks_closest_to_origin() {
        let mut app = setup_app();
        let e1 = spawn_entity(&mut app, Vec2::new(100.0, 0.0));
        let e2 = spawn_entity(&mut app, Vec2::new(10.0, 0.0));
        let e3 = spawn_entity(&mut app, Vec2::new(50.0, 0.0));
        let candidates = vec![
            (e1, Vec2::new(100.0, 0.0)),
            (e2, Vec2::new(10.0, 0.0)),
            (e3, Vec2::new(50.0, 0.0)),
        ];
        let ray_origin = Vec2::new(5.0, 0.0);
        let result = find_closest_to_ray(ray_origin, &candidates);
        assert_eq!(result, Some(e2));
    }

    #[test]
    fn resolve_interaction_type_farm_pot() {
        let mut app = setup_app();
        let entity = app.world_mut().spawn((Interactable, FarmPot)).id();
        let world = app.world();
        let result = resolve_interaction_type(entity, world);
        assert_eq!(result, InteractionType::FarmAction);
    }

    #[test]
    fn resolve_interaction_type_crafting_station() {
        let mut app = setup_app();
        let entity = app.world_mut().spawn((Interactable, CraftingStation)).id();
        let world = app.world();
        let result = resolve_interaction_type(entity, world);
        assert_eq!(result, InteractionType::Crafting);
    }

    #[test]
    fn resolve_interaction_type_boss_arena() {
        let mut app = setup_app();
        let entity = app.world_mut().spawn((Interactable, BossArenaEntry)).id();
        let world = app.world();
        let result = resolve_interaction_type(entity, world);
        assert_eq!(result, InteractionType::BossArena);
    }

    #[test]
    fn resolve_interaction_type_npc() {
        let mut app = setup_app();
        let entity = app.world_mut().spawn((Interactable, NPC)).id();
        let world = app.world();
        let result = resolve_interaction_type(entity, world);
        assert_eq!(result, InteractionType::NPC);
    }

    #[test]
    fn resolve_interaction_type_fallback() {
        let mut app = setup_app();
        let entity = app.world_mut().spawn(Interactable).id();
        let world = app.world();
        let result = resolve_interaction_type(entity, world);
        assert_eq!(result, InteractionType::FarmAction);
    }
}
