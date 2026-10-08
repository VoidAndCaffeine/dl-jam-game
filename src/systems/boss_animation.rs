use crate::components::boss::{Dying, PatternType};
use crate::components::boss_animation::{BossAnimState, BossAnimation};
use crate::components::player::Player;
use crate::components::player_sprite::{FRAME_COUNT, Facing8};
use crate::constants::BOSS_WALK_THRESHOLD;
use crate::events::BossAttackStarted;
use crate::resources::boss_sprite::{BossSpriteAssets, BossSpriteKey};
use crate::systems::boss_patterns::pattern_duration;
use crate::utils::targeting::{avoid_horizontal, snap_horizontal};
use bevy::ecs::message::MessageReader;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

/// The aim a pattern should actually use, after its direction constraints.
///
/// Tailings Surge only reads horizontally, and the Excavator Slam never reads
/// from the left or right, so both snap the raw player-aim before it is used
/// for movement or for choosing a clip.
pub fn constrained_aim(pattern: PatternType, aim: Vec2) -> Vec2 {
    match pattern {
        PatternType::TailingsSurge => snap_horizontal(aim),
        PatternType::ExcavatorSlam => avoid_horizontal(aim),
        _ => aim,
    }
}

/// The clip that plays for a pattern. Amalgamation has no art of its own, so it
/// reuses the Excavator's channel-heavy slam pose.
fn state_for_pattern(pattern: PatternType) -> BossAnimState {
    match pattern {
        PatternType::TailingsSurge => BossAnimState::TailingsSurge,
        PatternType::ExcavatorSlam => BossAnimState::ExcavatorSlam,
        PatternType::DebrisRain => BossAnimState::DebrisRain,
        PatternType::MirrorStep => BossAnimState::MirrorStep,
        PatternType::QuicksilverWave => BossAnimState::QuicksilverWave,
        PatternType::MadnessSpray => BossAnimState::Madness,
        PatternType::Amalgamation => BossAnimState::ExcavatorSlam,
    }
}

/// Turns attack events, death and movement into the boss's current clip.
pub fn drive_boss_animation(
    time: Res<Time>,
    mut events: MessageReader<BossAttackStarted>,
    player: Query<&Transform, With<Player>>,
    mut bosses: Query<(&mut BossAnimation, &Transform, Option<&Dying>)>,
) {
    let dt = time.delta_secs();
    let player_pos = player.single().map(|t| t.translation.truncate()).ok();

    // A committed pattern overrides locomotion, with its aim constrained.
    for event in events.read() {
        let Ok((mut anim, transform, dying)) = bosses.get_mut(event.entity) else {
            continue;
        };
        if dying.is_some() {
            continue;
        }
        let boss_pos = transform.translation.truncate();
        let aim = player_pos
            .map(|player| (player - boss_pos).normalize_or(Vec2::X))
            .unwrap_or(Vec2::X);
        let facing =
            Facing8::from_direction(constrained_aim(event.pattern, aim)).unwrap_or(anim.facing);
        anim.set_action(
            state_for_pattern(event.pattern),
            facing,
            pattern_duration(event.pattern, event.phase),
        );
    }

    // Dying bosses hold their death clip; everyone else returns to Idle or Walk.
    for (mut anim, transform, dying) in bosses.iter_mut() {
        let position = transform.translation.truncate();
        let previous = anim.last_position;
        anim.last_position = position;

        if dying.is_some() {
            anim.set_state(BossAnimState::Death);
            continue;
        }

        if anim.action_remaining > 0.0 {
            anim.action_remaining = (anim.action_remaining - dt).max(0.0);
            continue;
        }

        let travelled = position - previous;
        if travelled.length() > BOSS_WALK_THRESHOLD {
            anim.set_state(BossAnimState::Walk);
            if let Some(facing) = Facing8::from_direction(travelled) {
                anim.facing = facing;
            }
        } else {
            anim.set_state(BossAnimState::Idle);
        }
    }
}

/// Assets needed to draw the boss, all optional because headless tests run
/// without an asset server or atlas layout storage.
#[derive(SystemParam)]
pub struct BossSpriteAccess<'w> {
    cache: ResMut<'w, BossSpriteAssets>,
    server: Option<Res<'w, AssetServer>>,
    layouts: Option<ResMut<'w, Assets<TextureAtlasLayout>>>,
}

/// Advances each boss's clip and pushes the matching sheet section onto it.
pub fn animate_boss_sprite(
    time: Res<Time>,
    mut assets: BossSpriteAccess,
    mut bosses: Query<(&mut BossAnimation, &mut Sprite), Without<Player>>,
) {
    let dt = time.delta_secs();
    for (mut anim, mut sprite) in bosses.iter_mut() {
        anim.advance_frame(dt);
        apply_boss_sprite(&mut assets, &anim, &mut sprite);
    }
}

fn apply_boss_sprite(assets: &mut BossSpriteAccess, anim: &BossAnimation, sprite: &mut Sprite) {
    let Some(server) = assets.server.as_deref() else {
        return;
    };
    let Some(layouts) = assets.layouts.as_deref_mut() else {
        return;
    };

    if assets.cache.layout().is_none() {
        let handle = layouts.add(BossSpriteAssets::grid_layout());
        assets.cache.set_layout(handle);
    }
    let layout = assets
        .cache
        .layout()
        .expect("layout was just created")
        .clone();

    let key = BossSpriteKey::new(anim.pack, anim.state, anim.facing);
    sprite.image = assets.cache.image_for(key, server);
    sprite.texture_atlas = Some(TextureAtlas {
        layout,
        index: anim.frame.min(FRAME_COUNT - 1),
    });
    sprite.custom_size = Some(Vec2::splat(anim.pack.sprite_size()));
    sprite.color = Color::WHITE;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::boss::{Boss, BossId, Dying};
    use crate::components::boss_animation::BossPack;

    #[test]
    fn tailings_surge_is_snapped_to_pure_horizontal() {
        assert_eq!(
            constrained_aim(PatternType::TailingsSurge, Vec2::new(0.2, 0.98)),
            Vec2::X
        );
        assert_eq!(
            constrained_aim(PatternType::TailingsSurge, Vec2::new(-0.7, 0.7)),
            Vec2::NEG_X
        );
    }

    #[test]
    fn excavator_slam_avoids_left_and_right() {
        assert_eq!(
            constrained_aim(PatternType::ExcavatorSlam, Vec2::NEG_X),
            Vec2::Y
        );
        assert_eq!(
            constrained_aim(PatternType::ExcavatorSlam, Vec2::X),
            Vec2::Y
        );
        // Diagonals and vertical aims are left alone.
        assert_eq!(
            constrained_aim(PatternType::ExcavatorSlam, Vec2::new(0.7, 0.7)),
            Vec2::new(0.7, 0.7)
        );
        assert_eq!(
            constrained_aim(PatternType::ExcavatorSlam, Vec2::NEG_Y),
            Vec2::NEG_Y
        );
    }

    #[test]
    fn other_patterns_keep_their_aim() {
        let aim = Vec2::new(0.3, -0.6);
        assert_eq!(constrained_aim(PatternType::DebrisRain, aim), aim);
        assert_eq!(constrained_aim(PatternType::MirrorStep, aim), aim);
    }

    fn animation_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<BossAttackStarted>()
            .add_systems(Update, drive_boss_animation);
        app
    }

    fn spawn_boss(app: &mut App, pack: BossPack) -> Entity {
        app.world_mut()
            .spawn((
                BossAnimation::new(pack, Vec2::ZERO),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id()
    }

    #[test]
    fn a_surge_event_reads_horizontally_even_when_the_player_is_above() {
        let mut app = animation_app();
        let boss = spawn_boss(&mut app, BossPack::Excavator);
        app.world_mut()
            .spawn((Player, Transform::from_xyz(0.0, 100.0, 0.0)));
        app.world_mut().write_message(BossAttackStarted {
            entity: boss,
            boss_id: BossId::BossA,
            pattern: PatternType::TailingsSurge,
            phase: 1,
        });
        app.update();

        let anim = app.world().get::<BossAnimation>(boss).unwrap();
        assert_eq!(anim.state, BossAnimState::TailingsSurge);
        assert_eq!(anim.facing, Facing8::Right);
    }

    #[test]
    fn a_slam_event_never_reads_from_the_side() {
        let mut app = animation_app();
        let boss = spawn_boss(&mut app, BossPack::Excavator);
        // Player directly to the right, so the raw aim would be Facing8::Right.
        app.world_mut()
            .spawn((Player, Transform::from_xyz(100.0, 0.0, 0.0)));
        app.world_mut().write_message(BossAttackStarted {
            entity: boss,
            boss_id: BossId::BossA,
            pattern: PatternType::ExcavatorSlam,
            phase: 1,
        });
        app.update();

        let anim = app.world().get::<BossAnimation>(boss).unwrap();
        assert_eq!(anim.state, BossAnimState::ExcavatorSlam);
        assert_ne!(anim.facing, Facing8::Left);
        assert_ne!(anim.facing, Facing8::Right);
    }

    #[test]
    fn a_dying_boss_switches_to_its_death_clip() {
        let mut app = animation_app();
        let boss = spawn_boss(&mut app, BossPack::Mercuril);
        app.world_mut().entity_mut(boss).insert(Dying::new(1.0));
        app.update();

        let anim = app.world().get::<BossAnimation>(boss).unwrap();
        assert_eq!(anim.state, BossAnimState::Death);
    }

    #[test]
    fn a_moving_boss_walks_and_a_still_one_idles() {
        let mut app = animation_app();
        let boss = spawn_boss(&mut app, BossPack::Excavator);
        app.update();
        assert_eq!(
            app.world().get::<BossAnimation>(boss).unwrap().state,
            BossAnimState::Idle
        );

        app.world_mut()
            .get_mut::<Transform>(boss)
            .unwrap()
            .translation
            .x = 20.0;
        app.update();
        assert_eq!(
            app.world().get::<BossAnimation>(boss).unwrap().state,
            BossAnimState::Walk
        );
    }

    #[test]
    fn a_new_animation_starts_idle_at_its_spawn_point() {
        let anim = BossAnimation::new(BossPack::Mercuril, Vec2::new(4.0, 5.0));
        assert_eq!(anim.state, BossAnimState::Idle);
        assert_eq!(anim.last_position, Vec2::new(4.0, 5.0));
    }
}
