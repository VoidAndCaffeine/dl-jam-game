use crate::components::player::{Movement, Player};
use crate::components::player_sprite::{
    FRAME_COUNT, PLAYER_SPRITE_SIZE, PlayerAnimState, PlayerAnimation, PlayerLook, step_animation,
};
use crate::events::{CropPlanted, CropWatered, PlayerDied};
use crate::resources::level::ActiveLevel;
use crate::resources::player_attack_state::PlayerAttackState;
use crate::resources::player_sprite::{PlayerSpriteAssets, SpriteKey};
use crate::resources::run_data::PlayerGear;
use crate::states::Phase;
use bevy::ecs::message::MessageReader;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// Assets needed to draw the player, all optional because headless tests run
/// without an asset server or atlas layout storage.
#[derive(SystemParam)]
pub struct SpriteAssetAccess<'w> {
    cache: ResMut<'w, PlayerSpriteAssets>,
    server: Option<Res<'w, AssetServer>>,
    layouts: Option<ResMut<'w, Assets<TextureAtlasLayout>>>,
}

/// Picks the sprite pack for the current room and gear.
///
/// Farming always uses the farmer; arenas use the equipped armor's set, or the
/// farmer when nothing is equipped. A room change also resets the clip so the
/// player never walks into the next room mid-death.
pub fn update_player_look(
    active: Res<ActiveLevel>,
    gear: Res<PlayerGear>,
    mut players: Query<&mut PlayerAnimation, With<Player>>,
) {
    let look = PlayerLook::for_context(active.id, gear.armor.map(|piece| piece.set));
    for mut anim in players.iter_mut() {
        if anim.level != active.id {
            anim.level = active.id;
            anim.reset();
        }
        if anim.look != look {
            anim.look = look;
        }
    }
}

/// Turns farm interactions and death into one-shot clips.
pub fn collect_player_actions(
    mut planted: MessageReader<CropPlanted>,
    mut watered: MessageReader<CropWatered>,
    mut died: MessageReader<PlayerDied>,
    mut players: Query<&mut PlayerAnimation, With<Player>>,
) {
    let mut action: Option<PlayerAnimState> = None;
    for _ in planted.read() {
        action = Some(PlayerAnimState::Plant);
    }
    for _ in watered.read() {
        action = Some(PlayerAnimState::Water);
    }
    for _ in died.read() {
        action = Some(PlayerAnimState::Death);
    }

    let Some(action) = action else {
        return;
    };
    for mut anim in players.iter_mut() {
        // Only the farmer pack has planting and watering clips.
        if matches!(action, PlayerAnimState::Plant | PlayerAnimState::Water)
            && anim.look != PlayerLook::Farmer
        {
            continue;
        }
        anim.start_action(action);
    }
}

/// Advances the player's clip and pushes the matching sheet section onto the
/// sprite. Runs after `player_attack` so the swing type is up to date.
pub fn animate_player_sprite(
    time: Res<Time>,
    mut assets: SpriteAssetAccess,
    attack: Res<PlayerAttackState>,
    phase: Phase,
    mut players: Query<(&mut PlayerAnimation, &mut Sprite, &Movement), With<Player>>,
) {
    let dt = time.delta_secs();
    for (mut anim, mut sprite, movement) in players.iter_mut() {
        let attacking = if attack.is_rooted() {
            attack.current_attack
        } else {
            None
        };
        // In an arena the player faces the boss; otherwise it follows input.
        let facing = if phase.is_boss_fight() || attack.is_rooted() {
            attack.facing
        } else {
            movement.input_direction
        };

        step_animation(&mut anim, dt, facing, attacking);
        apply_sprite(&mut assets, &anim, &mut sprite);
    }
}

/// Ensures the shared layout exists, lazily loads the needed sheet and points
/// the sprite at the current frame.
fn apply_sprite(assets: &mut SpriteAssetAccess, anim: &PlayerAnimation, sprite: &mut Sprite) {
    let Some(server) = assets.server.as_deref() else {
        return;
    };
    let Some(layouts) = assets.layouts.as_deref_mut() else {
        return;
    };

    if assets.cache.layout().is_none() {
        let handle = layouts.add(PlayerSpriteAssets::grid_layout());
        assets.cache.set_layout(handle);
    }
    let layout = assets
        .cache
        .layout()
        .expect("layout was just created")
        .clone();

    let key = SpriteKey::new(anim.look, anim.state, anim.facing);
    sprite.image = assets.cache.image_for(key, server);
    sprite.texture_atlas = Some(TextureAtlas {
        layout,
        index: anim.frame.min(FRAME_COUNT - 1),
    });
    sprite.custom_size = Some(Vec2::splat(PLAYER_SPRITE_SIZE));
    sprite.color = Color::WHITE;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::gear::{GearPiece, GearSet, GearSlot};
    use crate::levels::LevelId;
    use crate::resources::level::PlayerSpawn;
    use crate::systems::spawn_player::spawn_player;
    use bevy::state::app::StatesPlugin;
    use bevy::transform::TransformPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ActiveLevel>()
            .init_resource::<PlayerGear>()
            .init_resource::<crate::levels::SolidGrid>()
            .init_resource::<PlayerSpawn>()
            .add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin))
            .init_state::<crate::states::GameState>()
            .init_state::<crate::states::DayPhase>()
            .add_message::<CropPlanted>()
            .add_message::<CropWatered>()
            .add_message::<PlayerDied>()
            .add_systems(Update, (update_player_look, collect_player_actions));
        app
    }

    fn spawn_player_with_animation(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((Player, PlayerAnimation::default()))
            .id()
    }

    fn look(app: &App, entity: Entity) -> PlayerLook {
        app.world().get::<PlayerAnimation>(entity).unwrap().look
    }

    fn set_level(app: &mut App, id: LevelId) {
        app.world_mut().resource_mut::<ActiveLevel>().id = id;
        app.update();
    }

    fn equip_armor(app: &mut App, set: GearSet) {
        app.world_mut().resource_mut::<PlayerGear>().armor =
            Some(GearPiece::new(set, GearSlot::Armor));
    }

    #[test]
    fn the_farm_always_wears_the_farmer_even_with_armour() {
        let mut app = setup_app();
        let player = spawn_player_with_animation(&mut app);
        equip_armor(&mut app, GearSet::Master);

        set_level(&mut app, LevelId::Farm);

        assert_eq!(look(&app, player), PlayerLook::Farmer);
    }

    #[test]
    fn an_arena_wears_the_equipped_armour() {
        let mut app = setup_app();
        let player = spawn_player_with_animation(&mut app);
        equip_armor(&mut app, GearSet::BossB);

        set_level(&mut app, LevelId::ArenaB);

        assert_eq!(look(&app, player), PlayerLook::BossB);
    }

    #[test]
    fn an_unarmoured_arena_wears_the_farmer() {
        let mut app = setup_app();
        let player = spawn_player_with_animation(&mut app);

        set_level(&mut app, LevelId::ArenaA);

        assert_eq!(look(&app, player), PlayerLook::Farmer);
    }

    #[test]
    fn changing_rooms_resets_the_clip() {
        let mut app = setup_app();
        let player = spawn_player_with_animation(&mut app);
        app.world_mut()
            .get_mut::<PlayerAnimation>(player)
            .unwrap()
            .start_action(PlayerAnimState::Death);

        set_level(&mut app, LevelId::ArenaA);

        let anim = app.world().get::<PlayerAnimation>(player).unwrap();
        assert_eq!(anim.state, PlayerAnimState::Idle);
        assert!(!anim.is_acting());
    }

    #[test]
    fn a_planted_crop_starts_the_plant_clip() {
        let mut app = setup_app();
        let player = spawn_player_with_animation(&mut app);

        app.world_mut()
            .write_message(CropPlanted(crate::components::pot::CropType::Starter));
        app.update();

        assert_eq!(
            app.world().get::<PlayerAnimation>(player).unwrap().state,
            PlayerAnimState::Plant
        );
    }

    #[test]
    fn watering_starts_the_water_clip() {
        let mut app = setup_app();
        let player = spawn_player_with_animation(&mut app);

        app.world_mut().write_message(CropWatered);
        app.update();

        assert_eq!(
            app.world().get::<PlayerAnimation>(player).unwrap().state,
            PlayerAnimState::Water
        );
    }

    #[test]
    fn death_starts_the_death_clip() {
        let mut app = setup_app();
        let player = spawn_player_with_animation(&mut app);

        app.world_mut().write_message(PlayerDied);
        app.update();

        assert_eq!(
            app.world().get::<PlayerAnimation>(player).unwrap().state,
            PlayerAnimState::Death
        );
    }

    #[test]
    fn farm_actions_are_ignored_outside_the_farmer_look() {
        let mut app = setup_app();
        let player = spawn_player_with_animation(&mut app);
        equip_armor(&mut app, GearSet::Starter);
        set_level(&mut app, LevelId::ArenaA);

        app.world_mut().write_message(CropWatered);
        app.update();

        assert!(
            !app.world()
                .get::<PlayerAnimation>(player)
                .unwrap()
                .is_acting(),
            "the starter pack has no watering clip"
        );
    }

    #[test]
    fn the_spawned_player_carries_the_animation_component() {
        let mut app = App::new();
        app.init_resource::<PlayerSpawn>()
            .init_resource::<PlayerGear>()
            .add_systems(Update, spawn_player);
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<&PlayerAnimation, With<Player>>();
        let anim = query.single(app.world()).unwrap();
        assert_eq!(anim.look, PlayerLook::Farmer);
    }
}
