use crate::components::attack::{AttackType, AttackVisual};
use crate::components::boss::{BOSS_SIZE, Boss};
use crate::components::player::{Movement, Player};
use crate::constants::{BOSS_HURTBOX_SCALE, LIGHT_WIDTH};
use crate::events::{BossDefeated, DamageDealt, HitConfirm};
use crate::resources::inventory_panel::InventoryPanel;
use crate::resources::player_attack_state::PlayerAttackState;
use crate::resources::run_data::PlayerGear;
use crate::states::Phase;
use crate::systems::combat::{damage_after_armor, swing_hits};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

/// Swing graphics sit in front of every world sprite.
const SWING_Z: f32 = 2.0;

/// Turns light and heavy input into a rectangular swing in front of the player.
///
/// In an arena the player always faces the nearest boss, so attacks land where
/// the player expects; elsewhere facing follows the last movement direction, so
/// a keyboard-only player can aim with WASD. The player is rooted for the
/// swing's duration by [`crate::resources::player_attack_state`].
#[allow(clippy::too_many_arguments)]
pub fn player_attack(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
    mut state: ResMut<PlayerAttackState>,
    gear: Res<PlayerGear>,
    inventory: Res<InventoryPanel>,
    player: Query<(&Transform, &Movement), With<Player>>,
    mut bosses: Query<(Entity, &mut Boss, &Transform)>,
    mut damage_events: MessageWriter<DamageDealt>,
    mut hit_events: MessageWriter<HitConfirm>,
    mut defeated: MessageWriter<BossDefeated>,
    phase: Phase,
) {
    state.tick(time.delta_secs());

    let Ok((transform, movement)) = player.single() else {
        return;
    };
    let origin = transform.translation.truncate();

    // Lock onto the nearest boss in an arena; follow WASD anywhere else.
    if phase.is_boss_fight() {
        if let Some(boss_pos) = nearest_boss_position(&bosses, origin) {
            state.facing = (boss_pos - origin).normalize_or(Vec2::X);
        }
    } else if movement.input_direction != Vec2::ZERO {
        state.facing = movement.input_direction.normalize();
    }

    if !phase.is_boss_fight() || inventory.open {
        return;
    }

    if state.is_rooted() {
        return;
    }

    let light = keys.just_pressed(KeyCode::KeyQ) || mouse.just_pressed(MouseButton::Left);
    let heavy = keys.just_pressed(KeyCode::KeyE) || mouse.just_pressed(MouseButton::Right);

    let attack = if heavy && state.is_ready(AttackType::Heavy) {
        Some(AttackType::Heavy)
    } else if light && state.is_ready(AttackType::Light) {
        Some(AttackType::Light)
    } else {
        None
    };
    let Some(attack) = attack else {
        return;
    };

    let stats = gear.stats();
    let (reach, width, raw_damage) = match attack {
        AttackType::Light => (
            stats.light_reach,
            LIGHT_WIDTH,
            stats.weapon_damage * attack.multiplier(),
        ),
        AttackType::Heavy => (
            stats.heavy_reach,
            stats.heavy_width,
            stats.weapon_damage * attack.multiplier(),
        ),
    };

    let facing = state.facing;
    let boss_half = Vec2::splat(BOSS_SIZE * 0.5 * BOSS_HURTBOX_SCALE);

    for (entity, mut boss, boss_transform) in bosses.iter_mut() {
        let boss_pos = boss_transform.translation.truncate();
        if !swing_hits(origin, facing, reach, width, boss_pos, boss_half) {
            continue;
        }

        let damage = damage_after_armor(raw_damage, 0.0);
        damage_events.write(DamageDealt {
            target: entity,
            amount: damage,
            raw: raw_damage,
        });
        hit_events.write(HitConfirm {
            target: entity,
            position: impact_point(origin, boss_pos, reach),
        });
        log::debug!("{} hit {:?} for {damage}", attack.label(), boss.id);

        if boss.damage(damage) {
            defeated.write(BossDefeated(boss.id));
            commands.entity(entity).despawn();
        }
    }

    spawn_swing(&mut commands, origin, facing, reach, width, attack);
    state.begin(attack);
}

/// The nearest boss's world position to `origin`.
fn nearest_boss_position(
    bosses: &Query<(Entity, &mut Boss, &Transform)>,
    origin: Vec2,
) -> Option<Vec2> {
    bosses
        .iter()
        .map(|(_, _, transform)| transform.translation.truncate())
        .min_by(|a, b| {
            a.distance_squared(origin)
                .total_cmp(&b.distance_squared(origin))
        })
}

/// Where the swing tip meets the boss, used to place the impact spark.
fn impact_point(origin: Vec2, boss_pos: Vec2, reach: f32) -> Vec2 {
    origin + (boss_pos - origin).clamp_length_max(reach)
}

/// Spawns the placeholder swing whose rectangle is the swing's hitbox.
fn spawn_swing(
    commands: &mut Commands,
    origin: Vec2,
    facing: Vec2,
    reach: f32,
    width: f32,
    attack: AttackType,
) {
    let facing = if facing == Vec2::ZERO {
        Vec2::X
    } else {
        facing.normalize()
    };
    let center = origin + facing * (reach * 0.5);
    let angle = facing.y.atan2(facing.x);
    let color = match attack {
        AttackType::Light => Color::srgba(0.55, 0.85, 1.0, 0.85),
        AttackType::Heavy => Color::srgba(1.0, 0.88, 0.45, 0.9),
    };

    commands.spawn((
        AttackVisual::new(attack),
        Sprite {
            color,
            custom_size: Some(Vec2::new(reach, width)),
            ..default()
        },
        Transform::from_translation(center.extend(SWING_Z))
            .with_rotation(Quat::from_rotation_z(angle)),
        Name::new(format!("{} Swing", attack.label())),
    ));
}

/// Despawns swing graphics once their lifetime runs out.
pub fn tick_attack_visuals(
    mut commands: Commands,
    time: Res<Time>,
    mut visuals: Query<(Entity, &mut AttackVisual)>,
) {
    let dt = time.delta_secs();
    for (entity, mut visual) in visuals.iter_mut() {
        visual.remaining -= dt;
        if visual.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::boss::BossId;
    use crate::states::{DayPhase, GameState};
    use bevy::ecs::message::MessageReader;
    use bevy::state::app::StatesPlugin;
    use bevy::transform::TransformPlugin;

    #[derive(Resource, Default)]
    struct CapturedDamage(Vec<DamageDealt>);

    #[derive(Resource, Default)]
    struct CapturedDefeats(Vec<BossDefeated>);

    #[derive(Resource, Default)]
    struct CapturedHits(Vec<HitConfirm>);

    fn capture_damage(
        mut captured: ResMut<CapturedDamage>,
        mut reader: MessageReader<DamageDealt>,
    ) {
        for event in reader.read() {
            captured.0.push(*event);
        }
    }

    fn capture_defeats(
        mut captured: ResMut<CapturedDefeats>,
        mut reader: MessageReader<BossDefeated>,
    ) {
        for event in reader.read() {
            captured.0.push(*event);
        }
    }

    fn capture_hits(mut captured: ResMut<CapturedHits>, mut reader: MessageReader<HitConfirm>) {
        for event in reader.read() {
            captured.0.push(*event);
        }
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<PlayerAttackState>()
            .init_resource::<PlayerGear>()
            .init_resource::<InventoryPanel>()
            .init_resource::<crate::resources::level::ActiveLevel>()
            .init_resource::<crate::resources::level::BossSpawn>()
            .init_resource::<crate::levels::SolidGrid>()
            .init_resource::<CapturedDamage>()
            .init_resource::<CapturedDefeats>()
            .init_resource::<CapturedHits>()
            .configure_sets(Update, crate::resources::level::LevelSet::Load)
            .add_plugins((
                MinimalPlugins,
                TransformPlugin,
                StatesPlugin,
                crate::plugins::boss::BossPlugin,
            ))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_systems(
                Update,
                (capture_damage, capture_defeats, capture_hits).after(player_attack),
            );

        app.world_mut().spawn((
            Player,
            Movement::default(),
            Transform::from_xyz(0.0, 0.0, 1.0),
        ));

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossFight);
        app.update();
        app
    }

    fn spawn_boss(app: &mut App, id: BossId, pos: Vec2) -> Entity {
        app.world_mut()
            .spawn((
                Boss::new_at(id, pos),
                Transform::from_xyz(pos.x, pos.y, 0.0),
            ))
            .id()
    }

    fn press(app: &mut App, key: KeyCode) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear_just_pressed(key);
    }

    fn boss_health(app: &App, entity: Entity) -> Option<f32> {
        app.world().get::<Boss>(entity).map(|boss| boss.health)
    }

    #[test]
    fn a_light_swing_damages_a_boss_in_front() {
        let mut app = setup_app();
        let boss = spawn_boss(&mut app, BossId::BossA, Vec2::new(50.0, 0.0));
        let full = crate::constants::BOSS_A_HEALTH;

        press(&mut app, KeyCode::KeyQ);

        assert_eq!(
            boss_health(&app, boss),
            Some(full - crate::constants::UNARMED_DAMAGE)
        );
        assert_eq!(app.world().resource::<CapturedDamage>().0.len(), 1);
    }

    #[test]
    fn a_hit_confirms_with_an_impact_position() {
        let mut app = setup_app();
        let boss = spawn_boss(&mut app, BossId::BossA, Vec2::new(50.0, 0.0));

        press(&mut app, KeyCode::KeyQ);

        let hits = &app.world().resource::<CapturedHits>().0;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].target, boss);
        assert!(
            hits[0].position.x > 0.0,
            "impact sits in front of the player"
        );
    }

    #[test]
    fn a_heavy_swing_hits_harder_than_a_light_one() {
        let mut app = setup_app();
        let boss = spawn_boss(&mut app, BossId::BossA, Vec2::new(50.0, 0.0));

        press(&mut app, KeyCode::KeyE);

        let expected = crate::constants::BOSS_A_HEALTH - crate::constants::UNARMED_DAMAGE * 1.8;
        assert_eq!(boss_health(&app, boss), Some(expected));
    }

    #[test]
    fn a_boss_out_of_reach_is_not_hit() {
        let mut app = setup_app();
        let boss = spawn_boss(&mut app, BossId::BossA, Vec2::new(500.0, 0.0));
        let full = crate::constants::BOSS_A_HEALTH;

        press(&mut app, KeyCode::KeyQ);

        assert_eq!(boss_health(&app, boss), Some(full));
        assert!(app.world().resource::<CapturedDamage>().0.is_empty());
    }

    #[test]
    fn auto_face_targets_the_nearest_boss() {
        let mut app = setup_app();
        let near = spawn_boss(&mut app, BossId::BossA, Vec2::new(40.0, 0.0));
        let behind = spawn_boss(&mut app, BossId::BossB, Vec2::new(-40.0, 0.0));

        press(&mut app, KeyCode::KeyQ);

        assert!(
            boss_health(&app, near) < Some(crate::constants::BOSS_A_HEALTH),
            "the nearest boss should be hit"
        );
        assert_eq!(
            boss_health(&app, behind),
            Some(crate::constants::BOSS_B_HEALTH),
            "the boss behind should be spared"
        );
    }

    #[test]
    fn a_swing_roots_the_player() {
        let mut app = setup_app();
        spawn_boss(&mut app, BossId::BossA, Vec2::new(50.0, 0.0));
        press(&mut app, KeyCode::KeyQ);
        assert!(app.world().resource::<PlayerAttackState>().is_rooted());
    }

    #[test]
    fn a_swing_spawns_a_matching_visual() {
        let mut app = setup_app();
        spawn_boss(&mut app, BossId::BossA, Vec2::new(50.0, 0.0));

        press(&mut app, KeyCode::KeyE);

        let mut visuals = app.world_mut().query::<&Sprite>();
        let sizes: Vec<Vec2> = visuals
            .iter(app.world())
            .filter_map(|sprite| sprite.custom_size)
            .collect();
        assert!(
            sizes
                .iter()
                .any(|size| size.x == crate::constants::UNARMED_HEAVY_REACH),
            "the swing sprite should match the swing's reach"
        );
    }

    #[test]
    fn attacking_outside_a_boss_fight_does_nothing() {
        let mut app = setup_app();
        let boss = spawn_boss(&mut app, BossId::BossA, Vec2::new(50.0, 0.0));
        let full = crate::constants::BOSS_A_HEALTH;
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);
        app.update();

        press(&mut app, KeyCode::KeyQ);

        assert_eq!(boss_health(&app, boss), Some(full));
    }

    #[test]
    fn a_lethal_blow_defeats_and_despawns_the_boss() {
        let mut app = setup_app();
        let boss = spawn_boss(&mut app, BossId::BossA, Vec2::new(50.0, 0.0));
        app.world_mut().get_mut::<Boss>(boss).unwrap().health = 1.0;

        press(&mut app, KeyCode::KeyQ);

        assert!(app.world().get::<Boss>(boss).is_none());
        let defeats = &app.world().resource::<CapturedDefeats>().0;
        assert_eq!(defeats.len(), 1);
        assert_eq!(defeats[0].0, BossId::BossA);
    }

    #[test]
    fn an_expired_visual_is_despawned() {
        let mut app = setup_app();
        let visual = app
            .world_mut()
            .spawn((
                AttackVisual {
                    attack_type: AttackType::Light,
                    remaining: 0.0,
                    total: 0.2,
                },
                Sprite::default(),
                Transform::default(),
            ))
            .id();

        app.update();

        assert!(app.world().get::<AttackVisual>(visual).is_none());
    }
}
