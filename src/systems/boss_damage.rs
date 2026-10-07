use crate::components::boss::{Boss, BossId, DualRole};
use crate::events::{BossDefeated, DamageDealt, PlaySfx, Sfx};
use crate::resources::boss_encounter::SharedBossHealth;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

/// Applies every queued hit to the boss it targeted.
///
/// Both the player's swings and friendly-fire debris go through here, so single
/// bosses, dual halves (one shared pool) and defeat all resolve in one place.
pub fn apply_boss_damage(
    mut commands: Commands,
    mut hits: MessageReader<DamageDealt>,
    mut bosses: Query<(&mut Boss, Option<&DualRole>)>,
    dual_halves: Query<Entity, With<DualRole>>,
    mut shared: ResMut<SharedBossHealth>,
    mut defeated: MessageWriter<BossDefeated>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let mut dual_damaged = false;

    for hit in hits.read() {
        let Ok((mut boss, role)) = bosses.get_mut(hit.target) else {
            continue;
        };

        if role.is_some() {
            // Ignore further halves once the shared pool is already empty this
            // frame, so both halves are only despawned once.
            if shared.current <= 0.0 {
                continue;
            }
            sfx.write(PlaySfx(Sfx::BossHit));
            dual_damaged = true;
            if shared.damage(hit.amount) {
                defeated.write(BossDefeated(BossId::Dual));
                for entity in dual_halves.iter() {
                    commands.entity(entity).despawn();
                }
                sfx.write(PlaySfx(Sfx::BossDefeated));
                sfx.write(PlaySfx(Sfx::MaterialDrop));
            }
        } else {
            // A boss already killed by an earlier hit this frame is skipped.
            if boss.is_dead() {
                continue;
            }
            sfx.write(PlaySfx(Sfx::BossHit));
            if boss.damage(hit.amount) {
                defeated.write(BossDefeated(boss.id));
                commands.entity(hit.target).despawn();
                sfx.write(PlaySfx(Sfx::BossDefeated));
                sfx.write(PlaySfx(Sfx::MaterialDrop));
            }
        }
    }

    // Keep each dual half's health bar in step with the shared pool.
    if dual_damaged {
        let fraction = shared.fraction();
        for (mut boss, role) in bosses.iter_mut() {
            if role.is_some() {
                boss.health = fraction * boss.max_health;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::DUAL_BOSS_HEALTH;

    fn app_with_dual() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_message::<DamageDealt>()
            .add_message::<BossDefeated>()
            .add_message::<PlaySfx>()
            .init_resource::<SharedBossHealth>()
            .add_systems(Update, apply_boss_damage);

        let excavator = app
            .world_mut()
            .spawn((
                Boss::new(BossId::Dual),
                DualRole::Excavator,
                Transform::default(),
            ))
            .id();
        let quicksilver = app
            .world_mut()
            .spawn((
                Boss::new(BossId::Dual),
                DualRole::Quicksilver,
                Transform::default(),
            ))
            .id();
        (app, excavator, quicksilver)
    }

    #[test]
    fn damaging_one_dual_half_feeds_the_shared_pool() {
        let (mut app, excavator, quicksilver) = app_with_dual();
        app.world_mut().write_message(DamageDealt {
            target: excavator,
            amount: 100.0,
            raw: 100.0,
        });
        app.update();

        let shared = app.world().resource::<SharedBossHealth>();
        assert_eq!(shared.current, DUAL_BOSS_HEALTH - 100.0);
        assert!(app.world().get::<Boss>(quicksilver).is_some());
    }

    #[test]
    fn emptying_the_shared_pool_drops_both_halves() {
        let (mut app, excavator, quicksilver) = app_with_dual();
        app.world_mut().write_message(DamageDealt {
            target: excavator,
            amount: DUAL_BOSS_HEALTH,
            raw: DUAL_BOSS_HEALTH,
        });
        app.update();

        assert!(app.world().get::<Boss>(excavator).is_none());
        assert!(app.world().get::<Boss>(quicksilver).is_none());
    }

    #[test]
    fn a_single_boss_dies_on_its_own_pool() {
        let mut app = App::new();
        app.add_message::<DamageDealt>()
            .add_message::<BossDefeated>()
            .add_message::<PlaySfx>()
            .init_resource::<SharedBossHealth>()
            .add_systems(Update, apply_boss_damage);
        let boss = app
            .world_mut()
            .spawn((Boss::new(BossId::BossA), Transform::default()))
            .id();

        app.world_mut().write_message(DamageDealt {
            target: boss,
            amount: 1.0,
            raw: 1.0,
        });
        app.update();
        assert!(app.world().get::<Boss>(boss).is_some());

        app.world_mut().write_message(DamageDealt {
            target: boss,
            amount: 10_000.0,
            raw: 10_000.0,
        });
        app.update();
        assert!(app.world().get::<Boss>(boss).is_none());
    }
}
