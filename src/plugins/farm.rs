use crate::components::collider::Collider;
use crate::components::pot::{Pot, PotState};
use crate::events::{CropHarvested, CropPlanted, CropWatered, DayAdvanced, InteractionEvent};
use crate::plugins::interaction::{FarmPot, HighlightMarker, Interactable};
use crate::resources::crop_select::CropSelectMenu;
use crate::resources::day_cycle::DayCycle;
use crate::resources::farm::{CropUnlocks, DayCounter, FarmState};
use crate::resources::inventory::Inventory;
use crate::resources::level::LevelSet;
use crate::states::{GameState, Phase};
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::prelude::*;

pub struct FarmPlugin;

impl Plugin for FarmPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DayCounter>()
            .init_resource::<CropUnlocks>()
            .init_resource::<FarmState>()
            .init_resource::<Inventory>()
            .init_resource::<CropSelectMenu>()
            .init_resource::<DayCycle>()
            .add_message::<CropPlanted>()
            .add_message::<CropWatered>()
            .add_message::<CropHarvested>()
            .add_message::<DayAdvanced>()
            .add_systems(OnExit(GameState::Playing), despawn_pots)
            .add_systems(FixedUpdate, pot_interaction_handler)
            .add_systems(FixedUpdate, update_pot_visuals)
            .add_systems(Update, begin_next_day.after(LevelSet::Load))
            .add_systems(Update, debug_advance_day.after(LevelSet::Load))
            .add_systems(Update, harvest_into_inventory)
            .add_systems(
                Update,
                snapshot_pots.after(begin_next_day).after(debug_advance_day),
            );
    }
}

pub const POT_SIZE: f32 = 40.0;

/// Spawns one pot at a world position. Position comes from the level's `p`
/// markers, and the index is the marker's order in the file.
pub fn spawn_pot(commands: &mut Commands, index: usize, position: Vec2) -> Entity {
    commands
        .spawn((
            Pot::new(index),
            Collider {
                size: Vec2::splat(POT_SIZE),
                is_solid: true,
            },
            Interactable::new(),
            FarmPot,
            Sprite {
                color: Color::srgb(1.0, 0.2, 0.2),
                custom_size: Some(Vec2::splat(POT_SIZE)),
                ..default()
            },
            Transform::from_xyz(position.x, position.y, 0.0),
            Name::new(format!("Pot {}", index)),
        ))
        .with_children(|parent| {
            parent.spawn((
                HighlightMarker,
                Sprite {
                    color: Color::srgba(1.0, 1.0, 0.0, 0.5),
                    custom_size: Some(Vec2::splat(POT_SIZE * 1.15)),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, crate::plugins::interaction::HIGHLIGHT_Z),
                Visibility::Hidden,
                Name::new("Highlight"),
            ));
            parent.spawn((
                Text2d::new(""),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::WHITE),
                Transform::from_xyz(0.0, POT_SIZE * 0.6, 1.0),
                Name::new("Day Counter Text"),
            ));
        })
        .id()
}

fn despawn_pots(mut commands: Commands, pots: Query<Entity, With<Pot>>) {
    for entity in pots.iter() {
        commands.entity(entity).despawn();
    }
}

fn pot_interaction_handler(
    mut events: MessageReader<InteractionEvent>,
    mut pots: Query<&mut Pot>,
    mut crop_select: ResMut<CropSelectMenu>,
    mut watered_events: MessageWriter<CropWatered>,
    mut harvested_events: MessageWriter<CropHarvested>,
    phase: Phase,
) {
    // Drain every event even outside farming so clicks made during a boss fight
    // can never be replayed against the pots once the farm is back.
    for event in events.read() {
        if !phase.is_farming() {
            continue;
        }
        let Ok(mut pot) = pots.get_mut(event.entity) else {
            continue;
        };

        match pot.state {
            PotState::Empty => {
                // Let the player choose which unlocked crop to plant.
                crop_select.open_menu(pot.index);
            }
            PotState::Planted => {
                if pot.water() {
                    watered_events.write(CropWatered);
                }
            }
            PotState::Watered => {
                // Already watered today - no action needed
            }
            PotState::Ready => {
                if let Some(crop) = pot.harvest() {
                    harvested_events.write(CropHarvested(crop));
                }
            }
        }
    }
}

fn harvest_into_inventory(
    mut events: MessageReader<CropHarvested>,
    mut inventory: ResMut<Inventory>,
) {
    for event in events.read() {
        inventory.add_crop(event.0, 1);
    }
}

fn begin_next_day(
    mut pots: Query<&mut Pot>,
    mut day_counter: ResMut<DayCounter>,
    mut day_cycle: ResMut<DayCycle>,
    mut day_advanced_events: MessageWriter<DayAdvanced>,
    phase: Phase,
) {
    if !phase.is_farming() || !day_cycle.pending_advance {
        return;
    }
    day_cycle.pending_advance = false;
    advance_day(&mut pots, &mut day_counter, &mut day_advanced_events);
}

fn debug_advance_day(
    mut pots: Query<&mut Pot>,
    mut day_counter: ResMut<DayCounter>,
    mut day_advanced_events: MessageWriter<DayAdvanced>,
    keys: Res<ButtonInput<KeyCode>>,
    phase: Phase,
) {
    if !phase.is_farming() {
        return;
    }
    if cfg!(debug_assertions) && keys.just_pressed(KeyCode::F9) {
        advance_day(&mut pots, &mut day_counter, &mut day_advanced_events);
    }
}

fn advance_day(
    pots: &mut Query<&mut Pot>,
    day_counter: &mut DayCounter,
    day_advanced_events: &mut MessageWriter<DayAdvanced>,
) {
    for mut pot in pots.iter_mut() {
        pot.advance_day();
        log::debug!(
            "pot {}: {:?} ({} days left)",
            pot.index,
            pot.state,
            pot.days_remaining
        );
    }
    day_counter.advance();
    log::info!("day advanced to {}", day_counter.0);
    day_advanced_events.write(DayAdvanced { day: day_counter.0 });
}

fn update_pot_visuals(
    mut pots: Query<(&Pot, &mut Sprite, &Children)>,
    mut text_query: Query<&mut Text2d>,
    phase: Phase,
) {
    if !phase.is_farming() {
        return;
    }
    for (pot, mut sprite, children) in pots.iter_mut() {
        sprite.color = pot.color();

        for child in children.iter() {
            if let Ok(mut text) = text_query.get_mut(child) {
                if pot.state == PotState::Empty || pot.days_remaining == 0 {
                    text.0.clear();
                } else {
                    text.0 = pot.days_remaining.to_string();
                }
            }
        }
    }
}

fn snapshot_pots(pots: Query<&Pot>, mut farm: ResMut<FarmState>) {
    if pots.is_empty() {
        return;
    }
    let mut snapshot: Vec<Pot> = pots.iter().copied().collect();
    snapshot.sort_by_key(|pot| pot.index);
    farm.pots = snapshot;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::pot::CropType;
    use crate::events::InteractionType;
    use crate::levels::LevelId;
    use crate::resources::level::LevelRequest;
    use crate::states::DayPhase;
    use bevy::state::app::StatesPlugin;
    use bevy::transform::TransformPlugin;

    const POT_COUNT: usize = 9;

    /// A bare app that runs the pot interaction handler in `Update` so tests do
    /// not depend on the fixed timestep ticking.
    fn setup_handler_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<CropSelectMenu>()
            .init_resource::<CropUnlocks>()
            .add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_message::<InteractionEvent>()
            .add_message::<CropWatered>()
            .add_message::<CropHarvested>()
            .add_systems(Update, pot_interaction_handler);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);
        app.update();
        app
    }

    fn interact(app: &mut App, entity: Entity) {
        app.world_mut().write_message(InteractionEvent {
            entity,
            interaction_type: InteractionType::FarmAction,
        });
        app.update();
    }

    #[test]
    fn interacting_with_an_empty_pot_opens_the_crop_picker_instead_of_planting() {
        let mut app = setup_handler_app();
        let pot = app.world_mut().spawn(Pot::new(2)).id();

        interact(&mut app, pot);

        let menu = app.world().resource::<CropSelectMenu>();
        assert!(menu.open, "the picker must open on an empty pot");
        assert_eq!(menu.pending_pot, 2);
        assert_eq!(
            app.world().get::<Pot>(pot).unwrap().state,
            PotState::Empty,
            "nothing is planted until the player chooses"
        );
    }

    #[test]
    fn interacting_with_a_planted_pot_still_waters_it() {
        let mut app = setup_handler_app();
        let pot = app.world_mut().spawn(Pot::new(0)).id();
        app.world_mut()
            .get_mut::<Pot>(pot)
            .unwrap()
            .plant(CropType::Starter);

        interact(&mut app, pot);

        assert_eq!(
            app.world().get::<Pot>(pot).unwrap().state,
            PotState::Watered
        );
        assert!(!app.world().resource::<CropSelectMenu>().open);
    }

    #[test]
    fn stale_boss_fight_clicks_do_not_replay_on_the_farm() {
        let mut app = setup_handler_app();
        let pot = app.world_mut().spawn(Pot::new(0)).id();

        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossFight);
        app.update();

        app.world_mut().write_message(InteractionEvent {
            entity: pot,
            interaction_type: InteractionType::FarmAction,
        });
        app.update();

        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);
        app.update();

        assert_eq!(
            app.world().get::<Pot>(pot).unwrap().state,
            PotState::Empty,
            "a click made during the boss fight must not affect the pot later"
        );
        assert!(
            !app.world().resource::<CropSelectMenu>().open,
            "the picker must not open from a stale click"
        );
    }

    fn setup_farm_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .add_plugins((
                MinimalPlugins,
                TransformPlugin,
                StatesPlugin,
                crate::plugins::level::LevelPlugin,
                FarmPlugin,
            ))
            .init_state::<GameState>()
            .init_state::<DayPhase>();
        app
    }

    fn enter_playing(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
    }

    fn exit_playing(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();
    }

    fn request_level(app: &mut App, id: LevelId) {
        app.world_mut().resource_mut::<LevelRequest>().0 = Some(id);
        app.update();
    }

    fn pot_indices(app: &mut App) -> Vec<usize> {
        let mut query = app.world_mut().query::<&Pot>();
        let mut indices: Vec<usize> = query.iter(app.world()).map(|pot| pot.index).collect();
        indices.sort_unstable();
        indices
    }

    #[test]
    fn farm_plugin_exists() {
        let _plugin = FarmPlugin;
    }

    #[test]
    fn pots_spawn_once_on_playing() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        assert_eq!(
            pot_indices(&mut app),
            (0..POT_COUNT).collect::<Vec<usize>>()
        );
    }

    #[test]
    fn pots_despawn_when_leaving_playing() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        assert_eq!(pot_indices(&mut app).len(), POT_COUNT);

        exit_playing(&mut app);
        assert!(pot_indices(&mut app).is_empty());
    }

    #[test]
    fn pots_do_not_duplicate_when_reentering_playing() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        exit_playing(&mut app);
        enter_playing(&mut app);

        assert_eq!(
            pot_indices(&mut app),
            (0..POT_COUNT).collect::<Vec<usize>>(),
            "re-entering Playing must not leave stale pots behind"
        );
    }

    #[test]
    fn pots_survive_day_phase_changes() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossSelect);
        app.update();

        assert_eq!(pot_indices(&mut app).len(), POT_COUNT);
    }

    #[test]
    fn a_fresh_farm_starts_with_empty_pots() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        let farm = app.world().resource::<FarmState>();
        assert_eq!(farm.pots.len(), POT_COUNT);
        assert!(farm.pots.iter().all(|pot| pot.state == PotState::Empty));
    }

    #[test]
    fn the_farm_state_tracks_planted_and_watered_pots() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        let pot = pot_entity(&mut app, 0);
        {
            let mut planted = app.world_mut().get_mut::<Pot>(pot).expect("pot entity");
            planted.plant(CropType::Starter);
            planted.water();
        }
        app.update();

        let farm = app.world().resource::<FarmState>();
        assert_eq!(farm.pots.len(), POT_COUNT);
        assert_eq!(farm.pots[0].state, PotState::Watered);
        assert!(farm.pots[0].watered_today);
        assert_eq!(farm.pots[1].state, PotState::Empty);
    }

    #[test]
    fn planted_crops_survive_a_boss_fight_round_trip() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        let pot = pot_entity(&mut app, 0);
        app.world_mut()
            .get_mut::<Pot>(pot)
            .expect("pot entity")
            .plant(CropType::CropA);
        app.update();

        request_level(&mut app, LevelId::ArenaA);
        assert!(pot_indices(&mut app).is_empty(), "the farm is left behind");

        request_level(&mut app, LevelId::Farm);
        let restored_index = pot_entity(&mut app, 0);
        let restored = app
            .world()
            .get::<Pot>(restored_index)
            .expect("pot is respawned");
        assert_eq!(restored.state, PotState::Watered);
        assert_eq!(restored.crop_type, CropType::CropA);
        assert_eq!(restored.days_remaining, CropType::CropA.growth_days());
    }

    #[test]
    fn watered_crops_advance_a_day_when_returning_from_a_boss_fight() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        let pot = pot_entity(&mut app, 0);
        {
            let mut planted = app.world_mut().get_mut::<Pot>(pot).expect("pot entity");
            planted.plant(CropType::Starter);
            planted.water();
        }
        app.update();

        request_level(&mut app, LevelId::ArenaA);
        app.world_mut().resource_mut::<DayCycle>().request_advance();
        request_level(&mut app, LevelId::Farm);

        let restored_index = pot_entity(&mut app, 0);
        let restored = app
            .world()
            .get::<Pot>(restored_index)
            .expect("pot is respawned");
        assert_eq!(restored.state, PotState::Planted);
        assert_eq!(restored.days_remaining, CropType::Starter.growth_days() - 1);
        assert!(!restored.watered_today);
    }

    #[test]
    fn harvested_pots_are_still_empty_after_returning_to_the_farm() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        let pot = grow_pot_to_ready(&mut app, 0);
        let crop = app
            .world_mut()
            .get_mut::<Pot>(pot)
            .expect("pot entity")
            .harvest()
            .expect("ripe pot yields a crop");
        app.world_mut().write_message(CropHarvested(crop));
        app.update();

        request_level(&mut app, LevelId::ArenaB);
        request_level(&mut app, LevelId::Farm);

        let restored_index = pot_entity(&mut app, 0);
        assert_eq!(
            app.world()
                .get::<Pot>(restored_index)
                .expect("pot is respawned")
                .state,
            PotState::Empty
        );
    }

    #[test]
    fn harvesting_a_ripe_pot_stocks_the_inventory() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        let pot = grow_pot_to_ready(&mut app, 0);
        let crop = app
            .world_mut()
            .get_mut::<Pot>(pot)
            .expect("pot entity")
            .harvest()
            .expect("ripe pot yields a crop");

        app.world_mut().write_message(CropHarvested(crop));
        app.update();

        let inventory = app.world().resource::<Inventory>();
        assert_eq!(inventory.crop_count(CropType::Starter), 1);
        assert_eq!(app.world().get::<Pot>(pot).unwrap().state, PotState::Empty);
    }

    #[test]
    fn harvests_accumulate_in_the_inventory() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        for _ in 0..3 {
            app.world_mut()
                .write_message(CropHarvested(CropType::Starter));
        }
        for _ in 0..2 {
            app.world_mut()
                .write_message(CropHarvested(CropType::CropA));
        }
        app.update();

        let inventory = app.world().resource::<Inventory>();
        assert_eq!(inventory.crop_count(CropType::Starter), 3);
        assert_eq!(inventory.crop_count(CropType::CropA), 2);
        assert_eq!(inventory.crop_count(CropType::CropB), 0);
    }

    #[test]
    fn harvesting_an_unripe_pot_yields_nothing_to_stock() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        let pot = pot_entity(&mut app, 1);
        app.world_mut()
            .get_mut::<Pot>(pot)
            .expect("pot 1 exists")
            .plant(CropType::Starter);

        let crop = app.world_mut().get_mut::<Pot>(pot).unwrap().harvest();
        assert_eq!(crop, None);
        app.update();

        assert_eq!(
            app.world()
                .resource::<Inventory>()
                .crop_count(CropType::Starter),
            0
        );
    }

    fn pot_entity(app: &mut App, index: usize) -> Entity {
        let mut query = app.world_mut().query_filtered::<Entity, With<Pot>>();
        query
            .iter(app.world())
            .find(|entity| {
                app.world()
                    .get::<Pot>(*entity)
                    .is_some_and(|pot| pot.index == index)
            })
            .expect("pot entity")
    }

    fn grow_pot_to_ready(app: &mut App, index: usize) -> Entity {
        let pot = pot_entity(app, index);
        let mut pots = app.world_mut().get_mut::<Pot>(pot).expect("pot entity");
        pots.plant(CropType::Starter);
        for _ in 0..CropType::Starter.growth_days() {
            pots.water();
            pots.advance_day();
        }
        pot
    }
}
