//! Small celebratory bursts of debris.
//!
//! Each burst is a handful of short-lived sprites that fly out, fall under
//! gravity and fade. They are purely cosmetic and headless-safe: a particle is
//! just a [`Sprite`], so a test without a renderer still sees the component and
//! its motion.

use crate::components::player::Player;
use crate::events::{CropHarvested, GearCrafted, HitConfirm};
use bevy::prelude::*;
use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};

/// Warm sparks for a landed hit.
const HIT_COLORS: [Color; 3] = [
    Color::srgb(1.0, 0.86, 0.3),
    Color::srgb(1.0, 0.55, 0.2),
    Color::srgb(1.0, 0.95, 0.7),
];
/// Leafy greens for a harvest.
const HARVEST_COLORS: [Color; 3] = [
    Color::srgb(0.45, 0.85, 0.35),
    Color::srgb(0.7, 0.95, 0.45),
    Color::srgb(0.3, 0.6, 0.25),
];
/// Bright metal-and-spark flecks for a craft.
const CRAFT_COLORS: [Color; 3] = [
    Color::srgb(0.95, 0.95, 1.0),
    Color::srgb(1.0, 0.8, 0.4),
    Color::srgb(0.7, 0.8, 0.95),
];

const PARTICLE_SIZE: f32 = 4.0;
const PARTICLE_Z: f32 = 6.0;

pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticleRng>()
            .add_message::<HitConfirm>()
            .add_message::<CropHarvested>()
            .add_message::<GearCrafted>()
            .add_systems(
                Update,
                (
                    spawn_hit_particles,
                    spawn_harvest_particles,
                    spawn_craft_particles,
                    tick_particles,
                ),
            );
    }
}

/// A cosmetic fleck that flies, falls and fades.
#[derive(Component, Debug)]
pub struct Particle {
    pub velocity: Vec2,
    pub gravity: f32,
    pub drag: f32,
    /// Total life, kept so the fade can be rebuilt each frame.
    pub lifetime: f32,
    pub remaining: f32,
    /// The un-faded colour.
    pub base: Color,
    /// Radians per second of spin.
    pub spin: f32,
}

impl Particle {
    fn alpha(&self) -> f32 {
        if self.lifetime <= 0.0 {
            0.0
        } else {
            (self.remaining / self.lifetime).clamp(0.0, 1.0)
        }
    }
}

/// Deterministic randomness for the burst spread.
#[derive(Resource)]
struct ParticleRng(SmallRng);

impl Default for ParticleRng {
    fn default() -> Self {
        Self(SmallRng::seed_from_u64(0x9E37_79B9_7F4A_7C15))
    }
}

impl ParticleRng {
    fn range(&mut self, min: f32, max: f32) -> f32 {
        self.0.random_range(min..max)
    }
}

/// The shape of one burst: how many flecks, how fast, how long, and how the
/// colour is picked.
struct Burst {
    count: u32,
    speed: (f32, f32),
    life: (f32, f32),
    gravity: f32,
    colors: &'static [Color],
}

/// Spawns a burst of particles radiating from `origin`.
fn spawn_burst(commands: &mut Commands, rng: &mut ParticleRng, origin: Vec2, burst: Burst) {
    for _ in 0..burst.count {
        let angle = rng.range(0.0, std::f32::consts::TAU);
        let speed = rng.range(burst.speed.0, burst.speed.1);
        let velocity = Vec2::new(angle.cos(), angle.sin()) * speed + Vec2::Y * speed * 0.4;
        let life = rng.range(burst.life.0, burst.life.1);
        let colors = burst.colors;
        let base = colors[(rng.range(0.0, colors.len() as f32) as usize).min(colors.len() - 1)];

        commands.spawn((
            Name::new("Particle"),
            Particle {
                velocity,
                gravity: burst.gravity,
                drag: 2.4,
                lifetime: life,
                remaining: life,
                base,
                spin: rng.range(-9.0, 9.0),
            },
            Sprite {
                color: base,
                custom_size: Some(Vec2::splat(PARTICLE_SIZE)),
                ..default()
            },
            Transform::from_xyz(origin.x, origin.y, PARTICLE_Z)
                .with_rotation(Quat::from_rotation_z(rng.range(0.0, std::f32::consts::TAU))),
        ));
    }
}

/// Sparks where a swing lands.
fn spawn_hit_particles(
    mut commands: Commands,
    mut rng: ResMut<ParticleRng>,
    mut hits: MessageReader<HitConfirm>,
) {
    for hit in hits.read() {
        spawn_burst(
            &mut commands,
            &mut rng,
            hit.position,
            Burst {
                count: 8,
                speed: (70.0, 175.0),
                life: (0.3, 0.5),
                gravity: 320.0,
                colors: &HIT_COLORS,
            },
        );
    }
}

/// A puff of leaves rising from the pot a crop came out of.
fn spawn_harvest_particles(
    mut commands: Commands,
    mut rng: ResMut<ParticleRng>,
    mut harvested: MessageReader<CropHarvested>,
) {
    for event in harvested.read() {
        spawn_burst(
            &mut commands,
            &mut rng,
            event.position,
            Burst {
                count: 10,
                speed: (55.0, 130.0),
                life: (0.4, 0.65),
                gravity: 260.0,
                colors: &HARVEST_COLORS,
            },
        );
    }
}

/// A shower of sparks over the farmer on a craft.
fn spawn_craft_particles(
    mut commands: Commands,
    mut rng: ResMut<ParticleRng>,
    mut crafted: MessageReader<GearCrafted>,
    players: Query<&Transform, With<Player>>,
) {
    let origin = players.single().ok().map(|t| t.translation.truncate());
    for _ in crafted.read() {
        if let Some(origin) = origin {
            spawn_burst(
                &mut commands,
                &mut rng,
                origin,
                Burst {
                    count: 12,
                    speed: (80.0, 190.0),
                    life: (0.35, 0.6),
                    gravity: 300.0,
                    colors: &CRAFT_COLORS,
                },
            );
        }
    }
}

/// Flies, falls and fades every live particle, despawning it at the end of its
/// life.
pub fn tick_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particles: Query<(Entity, &mut Particle, &mut Transform, &mut Sprite)>,
) {
    let dt = time.delta_secs();
    for (entity, mut particle, mut transform, mut sprite) in particles.iter_mut() {
        particle.remaining -= dt;
        if particle.remaining <= 0.0 {
            commands.entity(entity).despawn();
            continue;
        }

        particle.velocity.y -= particle.gravity * dt;
        let drag = (1.0 - particle.drag * dt).clamp(0.0, 1.0);
        particle.velocity *= drag;
        transform.translation.x += particle.velocity.x * dt;
        transform.translation.y += particle.velocity.y * dt;
        transform.rotate_z(particle.spin * dt);

        let base = particle.base.to_srgba();
        sprite.color = Color::srgba(base.red, base.green, base.blue, particle.alpha());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use bevy::transform::TransformPlugin;
    use std::time::Duration;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin))
            .init_resource::<ParticleRng>()
            .add_message::<HitConfirm>()
            .add_message::<CropHarvested>()
            .add_message::<GearCrafted>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                16,
            )))
            .add_systems(
                Update,
                (
                    spawn_hit_particles,
                    spawn_harvest_particles,
                    spawn_craft_particles,
                    tick_particles,
                ),
            );
        app.update();
        app
    }

    fn particles(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<Particle>>()
            .iter(app.world())
            .collect()
    }

    #[test]
    fn a_landed_hit_sprays_sparks() {
        let mut app = setup_app();
        app.world_mut().write_message(HitConfirm {
            target: Entity::PLACEHOLDER,
            position: Vec2::new(12.0, -3.0),
        });
        app.update();

        let spawned = particles(&mut app);
        assert_eq!(spawned.len(), 8, "one burst of eight");
        for entity in spawned {
            let transform = app.world().get::<Transform>(entity).unwrap();
            assert!(
                transform
                    .translation
                    .truncate()
                    .distance(Vec2::new(12.0, -3.0))
                    < 20.0,
                "spawns clustered at the impact"
            );
            assert!(app.world().get::<Sprite>(entity).is_some());
        }
    }

    #[test]
    fn a_harvest_bursts_from_the_pot() {
        let mut app = setup_app();
        // No player at all: the burst comes from the event's pot position.
        app.world_mut().write_message(CropHarvested {
            crop: crate::components::pot::CropType::Starter,
            position: Vec2::new(40.0, 8.0),
        });
        app.update();

        let spawned = particles(&mut app);
        assert_eq!(spawned.len(), 10);
        let transform = app.world().get::<Transform>(spawned[0]).unwrap();
        assert!(
            transform
                .translation
                .truncate()
                .distance(Vec2::new(40.0, 8.0))
                < 20.0
        );
    }

    #[test]
    fn a_craft_bursts_and_a_missing_player_is_simply_skipped() {
        let mut app = setup_app();
        // No player yet: the message is drained without spawning anything.
        app.world_mut()
            .write_message(GearCrafted(crate::components::gear::GearPiece::new(
                crate::components::gear::GearSet::Starter,
                crate::components::gear::GearSlot::Weapon,
            )));
        app.update();
        assert!(particles(&mut app).is_empty());

        app.world_mut()
            .spawn((Player, Transform::from_xyz(0.0, 0.0, 1.0)));
        app.world_mut()
            .write_message(GearCrafted(crate::components::gear::GearPiece::new(
                crate::components::gear::GearSet::Starter,
                crate::components::gear::GearSlot::Weapon,
            )));
        app.update();
        assert_eq!(particles(&mut app).len(), 12);
    }

    #[test]
    fn a_particle_falls_fades_and_despawns() {
        let mut app = setup_app();
        let entity = app
            .world_mut()
            .spawn((
                Particle {
                    velocity: Vec2::new(0.0, 60.0),
                    gravity: 400.0,
                    drag: 0.0,
                    lifetime: 0.2,
                    remaining: 0.2,
                    base: Color::WHITE,
                    spin: 0.0,
                },
                Sprite::default(),
                Transform::from_xyz(0.0, 0.0, PARTICLE_Z),
            ))
            .id();

        let start_y = app.world().get::<Transform>(entity).unwrap().translation.y;
        app.update();
        let risen = app.world().get::<Transform>(entity).unwrap().translation.y;
        assert!(risen > start_y, "the fleck rises on its initial velocity");

        for _ in 0..40 {
            app.update();
        }
        assert!(app.world().get::<Particle>(entity).is_none());
    }

    #[test]
    fn the_fade_tracks_the_remaining_life() {
        let mut app = setup_app();
        let entity = app
            .world_mut()
            .spawn((
                Particle {
                    velocity: Vec2::ZERO,
                    gravity: 0.0,
                    drag: 0.0,
                    lifetime: 1.0,
                    remaining: 0.25,
                    base: Color::WHITE,
                    spin: 0.0,
                },
                Sprite::default(),
                Transform::default(),
            ))
            .id();
        app.update();

        let color = app.world().get::<Sprite>(entity).unwrap().color;
        let alpha = color.to_srgba().alpha;
        assert!(alpha < 1.0, "a fading fleck is translucent: {alpha}");
    }
}
