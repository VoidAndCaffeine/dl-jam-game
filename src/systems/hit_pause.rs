//! Hit pause (hit stop).
//!
//! A landed hit or a boss defeat briefly freezes the world's movement and boss
//! AI. Only the fixed-step gameplay systems are gated; rendering, audio, UI and
//! particle effects keep running, so the freeze reads as impact rather than a
//! dropped frame.

use crate::events::{BossDefeated, HitConfirm};
use crate::resources::hit_pause::HitPause;
use crate::resources::settings::Settings;
use crate::systems::boss_ai::boss_ai;
use crate::systems::level_movement::grid_movement;
use bevy::prelude::*;
use bevy::time::Fixed;

/// Freeze on a landed player hit.
const HIT_PAUSE_HIT: f32 = 0.055;
/// A slightly longer freeze when a boss goes down.
const HIT_PAUSE_DEFEAT: f32 = 0.12;

pub struct HitPausePlugin;

impl Plugin for HitPausePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HitPause>()
            .add_message::<HitConfirm>()
            .add_message::<BossDefeated>()
            .add_systems(Update, trigger_hit_pause)
            // Tick before the gated systems so a freeze that ends this step lets
            // them run the same step.
            .add_systems(
                FixedUpdate,
                tick_hit_pause.before(grid_movement).before(boss_ai),
            );
    }
}

/// Turns impacts into a freeze request.
fn trigger_hit_pause(
    settings: Option<Res<Settings>>,
    mut pause: ResMut<HitPause>,
    mut hits: MessageReader<HitConfirm>,
    mut defeated: MessageReader<BossDefeated>,
) {
    // Reduced motion drains the events without freezing the world.
    let reduced = settings
        .as_ref()
        .is_some_and(|settings| settings.accessibility.reduced_motion);
    let hits = hits.read().count();
    let defeated = defeated.read().count();
    if reduced {
        return;
    }
    if hits > 0 {
        pause.trigger(HIT_PAUSE_HIT);
    }
    if defeated > 0 {
        pause.trigger(HIT_PAUSE_DEFEAT);
    }
}

/// Runs the freeze down on the fixed clock.
fn tick_hit_pause(time: Res<Time<Fixed>>, mut pause: ResMut<HitPause>) {
    pause.tick(time.delta_secs());
}

/// Run condition: the world may move this fixed step. Absent the resource (a
/// system wired up alone in a test), the world is never considered frozen.
pub fn world_unfrozen(pause: Option<Res<HitPause>>) -> bool {
    pause.map(|pause| !pause.is_frozen()).unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<HitPause>()
            .add_message::<HitConfirm>()
            .add_message::<BossDefeated>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                16,
            )))
            .add_systems(Update, trigger_hit_pause)
            .add_systems(FixedUpdate, tick_hit_pause);
        app.update();
        app
    }

    fn frozen(app: &App) -> bool {
        app.world().resource::<HitPause>().is_frozen()
    }

    #[test]
    fn a_landed_hit_freezes_the_world_briefly() {
        let mut app = setup_app();
        app.world_mut().write_message(HitConfirm {
            target: Entity::PLACEHOLDER,
            position: Vec2::ZERO,
        });
        app.update();
        assert!(frozen(&app), "a hit should freeze the world");

        // The freeze is short: it runs out within a few fixed steps.
        for _ in 0..20 {
            app.world_mut()
                .resource_mut::<Time<Fixed>>()
                .advance_by(Duration::from_millis(16));
            app.update();
        }
        assert!(!frozen(&app));
    }

    #[derive(Resource, Default)]
    struct Count(u32);

    fn bump(mut count: ResMut<Count>) {
        count.0 += 1;
    }

    #[test]
    fn the_run_condition_gates_a_system() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<HitPause>()
            .init_resource::<Count>()
            .add_systems(Update, bump.run_if(world_unfrozen));

        app.update();
        assert_eq!(app.world().resource::<Count>().0, 1, "runs while unfrozen");

        app.world_mut().resource_mut::<HitPause>().trigger(1.0);
        app.update();
        assert_eq!(app.world().resource::<Count>().0, 1, "blocked while frozen");

        app.world_mut().resource_mut::<HitPause>().tick(2.0);
        app.update();
        assert_eq!(
            app.world().resource::<Count>().0,
            2,
            "resumes after the thaw"
        );
    }

    #[test]
    fn without_the_resource_a_gated_system_still_runs() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Count>()
            .add_systems(Update, bump.run_if(world_unfrozen));
        app.update();
        assert_eq!(
            app.world().resource::<Count>().0,
            1,
            "a test without the resource never freezes"
        );
    }

    #[test]
    fn the_default_freeze_is_a_handful_of_frames() {
        const { assert!(HIT_PAUSE_HIT <= 0.1) };
        const { assert!(HIT_PAUSE_DEFEAT > HIT_PAUSE_HIT) };
    }
}
