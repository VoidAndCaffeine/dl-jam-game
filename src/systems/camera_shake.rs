//! Screen shake.
//!
//! Events add `trauma` to [`CameraShake`]; two systems bracket `camera_follow`
//! each fixed step. One peels off last step's offset so the follow spring reads
//! the camera's true position, the other bakes a fresh offset back in. Keeping
//! the peel and the bake on either side of the follow is what stops the shake
//! from feeding back into the spring as a persistent drift.

use crate::components::boss::PatternType;
use crate::components::player::Player;
use crate::events::{BossAttackStarted, BossDefeated, DamageDealt, HitConfirm, PlayerDied};
use crate::resources::camera_shake::{CameraShake, TRAUMA_DECAY, shake_offset};
use crate::systems::camera_follow::camera_follow;
use bevy::prelude::*;
use bevy::time::Fixed;
use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};

/// Trauma a landed player hit adds.
const HIT_TRAUMA: f32 = 0.16;
/// Trauma taking a hit adds; heavier than landing one, so a bad exchange reads.
const PLAYER_HIT_TRAUMA: f32 = 0.45;
/// Trauma a boss defeat adds.
const DEFEAT_TRAUMA: f32 = 0.6;
/// Trauma the player's own death adds.
const DEATH_TRAUMA: f32 = 0.9;

pub struct CameraShakePlugin;

impl Plugin for CameraShakePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraShake>()
            .init_resource::<ShakeNoise>()
            .add_message::<HitConfirm>()
            .add_message::<BossAttackStarted>()
            .add_message::<BossDefeated>()
            .add_message::<PlayerDied>()
            .add_message::<DamageDealt>()
            .add_systems(Update, accumulate_camera_trauma)
            // The peel runs before the follow, the bake after, so the spring
            // never integrates from a shaken position.
            .add_systems(FixedUpdate, clear_camera_shake.before(camera_follow))
            .add_systems(FixedUpdate, apply_camera_shake.after(camera_follow));
    }
}

/// Deterministic noise for the shake, so a replay and a test stay stable.
#[derive(Resource)]
struct ShakeNoise(SmallRng);

impl Default for ShakeNoise {
    fn default() -> Self {
        Self(SmallRng::seed_from_u64(0x5A4E_D0F0_1234_5678))
    }
}

impl ShakeNoise {
    /// A sample whose components run -1..1.
    fn sample(&mut self) -> Vec2 {
        Vec2::new(
            self.0.random_range(-1.0..1.0),
            self.0.random_range(-1.0..1.0),
        )
    }
}

/// The trauma a boss pattern contributes the moment it starts. Heavier,
/// screen-filling attacks shake harder than a quick reposition.
pub fn trauma_for_pattern(pattern: PatternType) -> f32 {
    match pattern {
        PatternType::ExcavatorSlam | PatternType::DebrisRain | PatternType::Amalgamation => 0.28,
        PatternType::TailingsSurge | PatternType::QuicksilverWave => 0.2,
        PatternType::MirrorStep | PatternType::MadnessSpray => 0.1,
    }
}

/// Turns combat events into trauma.
fn accumulate_camera_trauma(
    mut shake: ResMut<CameraShake>,
    mut hits: MessageReader<HitConfirm>,
    mut attacks: MessageReader<BossAttackStarted>,
    mut defeated: MessageReader<BossDefeated>,
    mut died: MessageReader<PlayerDied>,
    mut damage: MessageReader<DamageDealt>,
    players: Query<(), With<Player>>,
) {
    for _ in hits.read() {
        shake.add(HIT_TRAUMA);
    }
    for attack in attacks.read() {
        shake.add(trauma_for_pattern(attack.pattern));
    }
    for _ in defeated.read() {
        shake.add(DEFEAT_TRAUMA);
    }
    for _ in died.read() {
        shake.add(DEATH_TRAUMA);
    }
    // Boss damage also flows through `DamageDealt`; only the player's side shakes
    // here (landing one is already covered by the hit confirm above).
    for hit in damage.read() {
        if players.contains(hit.target) {
            shake.add(PLAYER_HIT_TRAUMA);
        }
    }
}

/// Peels the offset baked in last step back off the camera.
fn clear_camera_shake(
    mut shake: ResMut<CameraShake>,
    mut cameras: Query<&mut Transform, With<Camera2d>>,
) {
    let applied = shake.applied();
    if applied != Vec2::ZERO {
        for mut camera in cameras.iter_mut() {
            camera.translation.x -= applied.x;
            camera.translation.y -= applied.y;
        }
    }
    shake.set_applied(Vec2::ZERO);
}

/// Decays trauma and bakes a fresh offset into the camera for this step.
fn apply_camera_shake(
    time: Res<Time<Fixed>>,
    mut shake: ResMut<CameraShake>,
    mut noise: ResMut<ShakeNoise>,
    mut cameras: Query<&mut Transform, With<Camera2d>>,
) {
    let trauma = (shake.trauma() - TRAUMA_DECAY * time.delta_secs()).max(0.0);
    shake.set_trauma(trauma);

    let intensity = shake.intensity();
    let offset = if intensity > 0.0 {
        shake_offset(intensity, noise.sample())
    } else {
        Vec2::ZERO
    };

    if offset != Vec2::ZERO {
        for mut camera in cameras.iter_mut() {
            camera.translation.x += offset.x;
            camera.translation.y += offset.y;
        }
    }
    shake.set_applied(offset);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::boss::BossId;
    use crate::resources::camera_shake::MAX_SHAKE_OFFSET;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<CameraShake>()
            .init_resource::<ShakeNoise>()
            .add_message::<HitConfirm>()
            .add_message::<BossAttackStarted>()
            .add_message::<BossDefeated>()
            .add_message::<PlayerDied>()
            .add_message::<DamageDealt>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                16,
            )))
            .add_systems(Update, accumulate_camera_trauma)
            .add_systems(
                FixedUpdate,
                (clear_camera_shake, apply_camera_shake).chain(),
            );
        app.update();
        app
    }

    fn camera(app: &mut App) -> Entity {
        app.world_mut()
            .spawn((Camera2d, Transform::from_xyz(100.0, 50.0, 100.0)))
            .id()
    }

    fn shake(app: &App) -> &CameraShake {
        app.world().resource::<CameraShake>()
    }

    #[test]
    fn a_landed_hit_adds_trauma() {
        let mut app = setup_app();
        app.world_mut().write_message(HitConfirm {
            target: Entity::PLACEHOLDER,
            position: Vec2::ZERO,
        });
        app.update();
        assert!(shake(&app).trauma() > 0.0, "a hit should shake the camera");
    }

    #[test]
    fn only_player_damage_shakes_the_camera() {
        let mut app = setup_app();
        let player = app.world_mut().spawn(Player).id();
        let bystander = app.world_mut().spawn_empty().id();

        app.world_mut().write_message(DamageDealt {
            target: bystander,
            amount: 5.0,
            raw: 5.0,
        });
        app.update();
        assert_eq!(shake(&app).trauma(), 0.0, "boss damage does not shake here");

        app.world_mut().write_message(DamageDealt {
            target: player,
            amount: 12.0,
            raw: 20.0,
        });
        app.update();
        assert!(
            shake(&app).trauma() > 0.2,
            "taking a hit shakes harder than landing one"
        );
    }

    #[test]
    fn heavy_patterns_shake_harder_than_quick_ones() {
        assert!(
            trauma_for_pattern(PatternType::ExcavatorSlam)
                > trauma_for_pattern(PatternType::MirrorStep)
        );
        assert!(
            trauma_for_pattern(PatternType::Amalgamation)
                > trauma_for_pattern(PatternType::MadnessSpray)
        );
        for pattern in [
            PatternType::TailingsSurge,
            PatternType::ExcavatorSlam,
            PatternType::DebrisRain,
            PatternType::MirrorStep,
            PatternType::QuicksilverWave,
            PatternType::MadnessSpray,
            PatternType::Amalgamation,
        ] {
            assert!(trauma_for_pattern(pattern) > 0.0, "{pattern:?}");
        }
    }

    #[test]
    fn a_boss_defeat_and_player_death_both_shake() {
        let mut app = setup_app();
        app.world_mut().write_message(BossDefeated(BossId::BossA));
        app.world_mut().write_message(PlayerDied);
        app.update();
        assert!(shake(&app).trauma() > 0.5);
    }

    #[test]
    fn the_offset_is_baked_in_then_peeled_back_off() {
        let mut app = setup_app();
        let camera = camera(&mut app);
        app.world_mut().resource_mut::<CameraShake>().add(1.0);

        // One fixed step: the offset is baked onto the camera.
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .advance_by(Duration::from_millis(16));
        app.update();

        let shaken = app.world().get::<Transform>(camera).unwrap().translation;
        let applied = shake(&app).applied();
        assert_ne!(applied, Vec2::ZERO, "a full-trauma shake bakes an offset");
        assert!(
            (shaken.x - (100.0 + applied.x)).abs() < 1e-3,
            "the camera carries the offset"
        );

        // Run the trauma out, then one more step peels the offset back off.
        app.world_mut()
            .resource_mut::<CameraShake>()
            .set_trauma(0.0);
        for _ in 0..4 {
            app.world_mut()
                .resource_mut::<Time<Fixed>>()
                .advance_by(Duration::from_millis(16));
            app.update();
        }
        let rest = app.world().get::<Transform>(camera).unwrap().translation;
        assert_eq!(rest, Vec3::new(100.0, 50.0, 100.0), "no residual offset");
        assert_eq!(shake(&app).applied(), Vec2::ZERO);
    }

    #[test]
    fn trauma_decays_to_nothing() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<CameraShake>().add(1.0);
        for _ in 0..120 {
            app.world_mut()
                .resource_mut::<Time<Fixed>>()
                .advance_by(Duration::from_millis(16));
            app.update();
        }
        assert_eq!(shake(&app).trauma(), 0.0);
    }

    #[test]
    fn the_offset_never_exceeds_the_configured_maximum() {
        let mut app = setup_app();
        let camera = camera(&mut app);
        app.world_mut().resource_mut::<CameraShake>().add(1.0);
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .advance_by(Duration::from_millis(16));
        app.update();

        let pos = app.world().get::<Transform>(camera).unwrap().translation;
        let offset = Vec2::new(pos.x - 100.0, pos.y - 50.0);
        assert!(
            offset.length() <= MAX_SHAKE_OFFSET * 1.4143,
            "offset {offset:?} exceeded the bound"
        );
    }
}
