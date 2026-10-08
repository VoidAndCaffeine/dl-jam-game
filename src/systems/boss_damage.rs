use crate::components::boss::{Boss, BossId, DualRole, Dying, SurgeCharger};
use crate::components::boss_animation::{BossPack, death_duration};
use crate::events::{BossDefeated, DamageDealt, PlaySfx, Sfx};
use crate::resources::boss_encounter::{BossCoordinator, SharedBossHealth};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

/// Applies every queued hit to the boss it targeted.
///
/// Both the player's swings and friendly-fire debris go through here. A lethal
/// hit does not despawn the boss: it starts the death clip via [`Dying`], and
/// [`tick_dying_bosses`] announces the defeat once that clip finishes.
pub fn apply_boss_damage(
    mut commands: Commands,
    mut hits: MessageReader<DamageDealt>,
    mut bosses: Query<(Entity, &mut Boss, Option<&DualRole>, Option<&Dying>)>,
    mut shared: ResMut<SharedBossHealth>,
    mut coordinator: ResMut<BossCoordinator>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let mut dual_touched = false;
    let mut dual_lethal = false;

    for hit in hits.read() {
        let Ok((_, mut boss, role, dying)) = bosses.get_mut(hit.target) else {
            continue;
        };
        // A boss already dying is immune to further hits.
        if dying.is_some() {
            continue;
        }

        if role.is_some() {
            // Ignore further halves once the shared pool is already empty this
            // frame, so the death is only started once.
            if shared.current <= 0.0 {
                continue;
            }
            sfx.write(PlaySfx(Sfx::BossHit));
            dual_touched = true;
            if shared.damage(hit.amount) {
                dual_lethal = true;
            }
        } else if boss.is_dead() {
            // A boss already killed by an earlier hit this frame is skipped.
            continue;
        } else {
            sfx.write(PlaySfx(Sfx::BossHit));
            if boss.damage(hit.amount) {
                start_death(&mut commands, hit.target, BossPack::from_id(boss.id));
            }
        }
    }

    // Keep each dual half's health bar in step with the shared pool.
    if dual_touched {
        let fraction = shared.fraction();
        for (_, mut boss, role, _) in bosses.iter_mut() {
            if role.is_some() {
                boss.health = fraction * boss.max_health;
            }
        }
    }

    // Both dual halves die together, each playing its own pack's death sheet.
    if dual_lethal {
        coordinator.end();
        for (entity, _, role, dying) in bosses.iter_mut() {
            if let Some(role) = role
                && dying.is_none()
            {
                start_death(&mut commands, entity, BossPack::from_role(*role));
            }
        }
    }
}

/// Puts a boss into its death clip and stops it mid-charge.
fn start_death(commands: &mut Commands, entity: Entity, pack: BossPack) {
    commands
        .entity(entity)
        .insert(Dying::new(death_duration(pack)));
    // A dying boss should stop charging mid-sprint.
    commands.entity(entity).remove::<SurgeCharger>();
}

/// Runs down death clips and announces the defeat once they finish.
pub fn tick_dying_bosses(
    mut commands: Commands,
    time: Res<Time>,
    mut dying: Query<(Entity, &mut Dying, &Boss, Option<&DualRole>)>,
    dual_halves: Query<Entity, With<DualRole>>,
    mut defeated: MessageWriter<BossDefeated>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let dt = time.delta_secs();
    let mut dual_done = false;
    let mut singles: Vec<(Entity, BossId)> = Vec::new();

    for (entity, mut death, boss, role) in dying.iter_mut() {
        if death.remaining > 0.0 {
            death.remaining = (death.remaining - dt).max(0.0);
        }
        if death.remaining <= 0.0 {
            if role.is_some() {
                dual_done = true;
            } else {
                singles.push((entity, boss.id));
            }
        }
    }

    if dual_done {
        for entity in dual_halves.iter() {
            commands.entity(entity).despawn();
        }
        defeated.write(BossDefeated(BossId::Dual));
        sfx.write(PlaySfx(Sfx::BossDefeated));
        sfx.write(PlaySfx(Sfx::MaterialDrop));
    }

    for (entity, id) in singles {
        commands.entity(entity).despawn();
        defeated.write(BossDefeated(id));
        sfx.write(PlaySfx(Sfx::BossDefeated));
        sfx.write(PlaySfx(Sfx::MaterialDrop));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::DUAL_BOSS_HEALTH;

    fn base_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<DamageDealt>()
            .add_message::<BossDefeated>()
            .add_message::<PlaySfx>()
            .init_resource::<SharedBossHealth>()
            .init_resource::<BossCoordinator>()
            .add_systems(Update, (apply_boss_damage, tick_dying_bosses).chain());
        app
    }

    #[test]
    fn a_lethal_hit_starts_a_death_clip_before_the_defeat() {
        let mut app = base_app();
        let boss = app
            .world_mut()
            .spawn((Boss::new(BossId::BossA), Transform::default()))
            .id();

        app.world_mut().write_message(DamageDealt {
            target: boss,
            amount: 10_000.0,
            raw: 10_000.0,
        });
        app.update();

        // Still alive, playing the death clip.
        assert!(app.world().get::<Boss>(boss).is_some());
        assert!(app.world().get::<Dying>(boss).is_some());

        // Let the death timer run out.
        app.world_mut().get_mut::<Dying>(boss).unwrap().remaining = 0.0;
        app.update();
        assert!(app.world().get::<Boss>(boss).is_none());
    }

    #[test]
    fn a_dying_boss_ignores_further_hits() {
        let mut app = base_app();
        let boss = app
            .world_mut()
            .spawn((Boss::new(BossId::BossA), Transform::default()))
            .id();
        app.world_mut().write_message(DamageDealt {
            target: boss,
            amount: 10_000.0,
            raw: 10_000.0,
        });
        app.update();

        let before = app.world().get::<Dying>(boss).unwrap().remaining;
        app.world_mut().write_message(DamageDealt {
            target: boss,
            amount: 500.0,
            raw: 500.0,
        });
        app.update();
        // The death clock keeps running down; the extra hit does not restart it.
        assert!(
            app.world().get::<Dying>(boss).unwrap().remaining <= before,
            "a dying boss should ignore further hits"
        );
    }

    fn app_with_dual() -> (App, Entity, Entity) {
        let mut app = base_app();
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
    fn emptying_the_shared_pool_starts_both_deaths() {
        let (mut app, excavator, quicksilver) = app_with_dual();
        app.world_mut().write_message(DamageDealt {
            target: excavator,
            amount: DUAL_BOSS_HEALTH,
            raw: DUAL_BOSS_HEALTH,
        });
        app.update();

        assert!(app.world().get::<Dying>(excavator).is_some());
        assert!(app.world().get::<Dying>(quicksilver).is_some());
    }

    #[test]
    fn a_finished_dual_death_drops_both_halves() {
        let (mut app, excavator, quicksilver) = app_with_dual();
        app.world_mut().write_message(DamageDealt {
            target: excavator,
            amount: DUAL_BOSS_HEALTH,
            raw: DUAL_BOSS_HEALTH,
        });
        app.update();
        for entity in [excavator, quicksilver] {
            app.world_mut().get_mut::<Dying>(entity).unwrap().remaining = 0.0;
        }
        app.update();

        assert!(app.world().get::<Boss>(excavator).is_none());
        assert!(app.world().get::<Boss>(quicksilver).is_none());
    }
}
