//! Floating damage numbers.
//!
//! Every [`DamageDealt`] spawns a short-lived number above the damaged entity
//! that rises and fades. Boss damage reads white, player damage red, so a glance
//! tells who is winning the exchange.

use crate::components::boss::Boss;
use crate::components::player::Player;
use crate::events::DamageDealt;
use bevy::ecs::message::MessageReader;
use bevy::prelude::*;

/// How long a number stays on screen, and how far it rises over its life.
const LIFETIME: f32 = 0.9;
const RISE_SPEED: f32 = 46.0;
/// Where the number starts, above the entity's centre.
const SPAWN_OFFSET: f32 = 26.0;
const FONT_SIZE: f32 = 16.0;
const Z: f32 = 12.0;

const BOSS_COLOR: Color = Color::WHITE;
const PLAYER_COLOR: Color = Color::srgb(1.0, 0.2, 0.2);

/// A rising, fading number. `elapsed` runs from 0 to [`LIFETIME`]; `base` keeps
/// the un-faded colour so the alpha can be rebuilt each frame.
#[derive(Component)]
pub struct DamageNumber {
    elapsed: f32,
    base: Color,
}

/// Spawns a number above every damaged boss or player.
pub fn spawn_damage_numbers(
    mut commands: Commands,
    mut hits: MessageReader<DamageDealt>,
    targets: Query<(&Transform, Option<&Boss>, Option<&Player>)>,
) {
    for hit in hits.read() {
        let Ok((transform, boss, player)) = targets.get(hit.target) else {
            continue;
        };
        let base = if player.is_some() {
            PLAYER_COLOR
        } else if boss.is_some() {
            BOSS_COLOR
        } else {
            continue;
        };
        let position = transform.translation.truncate() + Vec2::Y * SPAWN_OFFSET;
        commands.spawn((
            DamageNumber { elapsed: 0.0, base },
            Text2d::new(format!("{}", hit.amount.round().max(0.0) as i32)),
            TextFont {
                font_size: FontSize::Px(FONT_SIZE),
                ..default()
            },
            TextColor(base),
            Transform::from_translation(position.extend(Z)),
            Name::new("Damage Number"),
        ));
    }
}

/// Rises and fades every live number, despawning it once its life runs out.
pub fn tick_damage_numbers(
    mut commands: Commands,
    time: Res<Time>,
    mut numbers: Query<(Entity, &mut DamageNumber, &mut Transform, &mut TextColor)>,
) {
    let dt = time.delta_secs();
    for (entity, mut number, mut transform, mut color) in numbers.iter_mut() {
        number.elapsed += dt;
        let progress = (number.elapsed / LIFETIME).clamp(0.0, 1.0);
        transform.translation.y += RISE_SPEED * dt;
        let base = number.base.to_srgba();
        color.0 = Color::srgba(base.red, base.green, base.blue, 1.0 - progress);
        if number.elapsed >= LIFETIME {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::boss::BossId;
    use crate::components::player::Health;
    use bevy::time::TimeUpdateStrategy;
    use bevy::transform::TransformPlugin;
    use std::time::Duration;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin))
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                50,
            )))
            .add_message::<DamageDealt>()
            .add_systems(Update, (tick_damage_numbers, spawn_damage_numbers).chain());
        // Prime real time so later frames have a real delta.
        app.update();
        app
    }

    fn numbers(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<DamageNumber>>()
            .iter(app.world())
            .collect()
    }

    #[test]
    fn boss_damage_spawns_a_white_number_above_the_boss() {
        let mut app = setup_app();
        let boss = app
            .world_mut()
            .spawn((
                Boss::new_at(BossId::BossA, Vec2::new(10.0, 0.0)),
                Transform::from_xyz(10.0, 0.0, 0.0),
            ))
            .id();

        app.world_mut().write_message(DamageDealt {
            target: boss,
            amount: 25.0,
            raw: 25.0,
        });
        app.update();

        let numbers = numbers(&mut app);
        assert_eq!(numbers.len(), 1);
        let transform = app.world().get::<Transform>(numbers[0]).unwrap();
        assert_eq!(
            transform.translation.truncate(),
            Vec2::new(10.0, SPAWN_OFFSET)
        );
        assert_eq!(
            app.world().get::<TextColor>(numbers[0]).unwrap().0,
            BOSS_COLOR
        );
    }

    #[test]
    fn player_damage_spawns_a_red_number() {
        let mut app = setup_app();
        let player = app
            .world_mut()
            .spawn((Player, Health::new(100.0, 0.0), Transform::default()))
            .id();

        app.world_mut().write_message(DamageDealt {
            target: player,
            amount: 7.4,
            raw: 10.0,
        });
        app.update();

        let numbers = numbers(&mut app);
        assert_eq!(numbers.len(), 1);
        assert_eq!(
            app.world().get::<TextColor>(numbers[0]).unwrap().0,
            PLAYER_COLOR
        );
    }

    #[test]
    fn damage_to_a_plain_target_is_ignored() {
        let mut app = setup_app();
        let bystander = app.world_mut().spawn(Transform::default()).id();

        app.world_mut().write_message(DamageDealt {
            target: bystander,
            amount: 5.0,
            raw: 5.0,
        });
        app.update();

        assert!(numbers(&mut app).is_empty());
    }

    #[test]
    fn a_number_rises_fades_and_despawns() {
        let mut app = setup_app();
        let boss = app
            .world_mut()
            .spawn((
                Boss::new_at(BossId::BossA, Vec2::ZERO),
                Transform::default(),
            ))
            .id();
        app.world_mut().write_message(DamageDealt {
            target: boss,
            amount: 3.0,
            raw: 3.0,
        });
        app.update();
        let number = numbers(&mut app)[0];
        let start_y = app.world().get::<Transform>(number).unwrap().translation.y;

        app.update();
        let risen = app.world().get::<Transform>(number).unwrap().translation.y;
        assert!(risen > start_y, "the number rises as it ages");

        for _ in 0..20 {
            app.update();
        }
        assert!(app.world().get::<DamageNumber>(number).is_none());
    }
}
