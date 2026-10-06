use crate::components::boss::{BOSS_SIZE, Boss};
use crate::constants::BOSS_WANDER_SPEED;
use crate::events::BossPhaseChanged;
use crate::levels::grid::SolidGrid;
use crate::states::Phase;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

/// Placeholder boss brain: wanders around its spawn point, stands still and
/// flashes white while a phase change stuns it, and moves faster in phase 2.
///
/// The interesting decisions live on [`Boss::wander_step`], so replacing this
/// with real attack patterns does not touch movement, rendering or damage.
pub fn boss_ai(
    grid: Res<SolidGrid>,
    time: Res<Time<Fixed>>,
    mut bosses: Query<(&mut Boss, &mut Transform, &mut Sprite)>,
    mut changed: MessageWriter<BossPhaseChanged>,
    phase: Phase,
) {
    if !phase.is_boss_fight() {
        return;
    }

    let dt = time.delta_secs();
    let half = Vec2::splat(BOSS_SIZE * 0.5);

    for (mut boss, mut transform, mut sprite) in bosses.iter_mut() {
        let previous_phase = boss.phase;
        let target = boss.wander_step(dt);
        if boss.phase != previous_phase {
            changed.write(BossPhaseChanged {
                boss_id: boss.id,
                new_phase: boss.phase,
            });
        }

        // A stunned boss flashes white so the phase change reads on screen.
        sprite.color = if boss.stun_remaining > 0.0 {
            Color::WHITE
        } else {
            boss.id.color()
        };

        let Some(target) = target else {
            continue;
        };
        let from = transform.translation.truncate();
        let speed = BOSS_WANDER_SPEED * boss.speed_multiplier();
        let step = (target - from).clamp_length_max(speed * dt);
        if step == Vec2::ZERO {
            continue;
        }
        let moved = grid.move_and_collide(half, from, step);
        transform.translation.x = moved.x;
        transform.translation.y = moved.y;
    }
}
