//! Every combat tuning value lives here so balancing is a one-file job.

// --- Player ---

/// Health with no armor equipped.
pub const PLAYER_BASE_HEALTH: f32 = 80.0;

/// Total max health granted by an armor piece, indexed by `GearSet`:
/// Starter, Boss A, Boss B, Master.
pub const SET_MAX_HEALTH: [f32; 4] = [100.0, 130.0, 130.0, 170.0];

/// Flat damage reduction granted by an armor piece, indexed by `GearSet`.
pub const SET_ARMOR: [f32; 4] = [5.0, 10.0, 10.0, 15.0];

/// Weapon damage, indexed by `GearSet`.
pub const SET_DAMAGE: [f32; 4] = [25.0, 35.0, 35.0, 50.0];

/// Reach of a light (poke) swing, indexed by `GearSet`.
pub const SET_LIGHT_REACH: [f32; 4] = [70.0, 85.0, 85.0, 95.0];

/// Pokes are long and narrow, so one width covers every set.
pub const LIGHT_WIDTH: f32 = 28.0;

/// Reach of a heavy (slash) swing, indexed by `GearSet`.
pub const SET_HEAVY_REACH: [f32; 4] = [55.0, 70.0, 70.0, 80.0];

/// Width of a heavy (slash) swing, indexed by `GearSet`.
pub const SET_HEAVY_WIDTH: [f32; 4] = [50.0, 70.0, 70.0, 80.0];

/// Bare-handed fallbacks used when no weapon is equipped.
pub const UNARMED_DAMAGE: f32 = 15.0;
pub const UNARMED_LIGHT_REACH: f32 = 55.0;
pub const UNARMED_HEAVY_REACH: f32 = 45.0;
pub const UNARMED_HEAVY_WIDTH: f32 = 45.0;

/// Light attacks are fast and exact; heavy attacks hit harder and wider.
pub const LIGHT_MULTIPLIER: f32 = 1.0;
pub const HEAVY_MULTIPLIER: f32 = 1.8;
pub const LIGHT_COOLDOWN: f32 = 0.35;
pub const HEAVY_COOLDOWN: f32 = 0.9;

/// How long the player is rooted and the placeholder swing is drawn.
pub const ATTACK_DURATION: f32 = 0.2;

// --- Bosses ---

pub const BOSS_A_HEALTH: f32 = 450.0;
pub const BOSS_B_HEALTH: f32 = 500.0;
/// The dual boss fights from a single shared pool.
pub const DUAL_BOSS_HEALTH: f32 = 900.0;

/// Bosses get a hurtbox slightly larger than their sprite so near-misses still
/// connect during a fast fight.
pub const BOSS_HURTBOX_SCALE: f32 = 1.1;

/// Fraction of max health at which a boss enters phase 2.
pub const PHASE_THRESHOLD: f32 = 0.5;
/// How long a boss stands still when its phase changes.
pub const PHASE_STUN_DURATION: f32 = 1.0;

/// Placeholder wander movement. Phase 2 multiplies speed and angular speed.
pub const BOSS_WANDER_SPEED: f32 = 60.0;
pub const BOSS_WANDER_ANGULAR_SPEED: f32 = 0.8;
pub const BOSS_WANDER_RADIUS: f32 = 80.0;
pub const BOSS_PHASE2_MULTIPLIER: f32 = 1.6;

// --- Boss sizes ---

/// The Excavator is a hulking industrial-organic hybrid.
pub const BOSS_A_SIZE: f32 = 96.0;
/// The Quicksilver is a slighter, fluid horror.
pub const BOSS_B_SIZE: f32 = 80.0;

/// The sprite size of a boss id. Both dual halves keep their native sizes.
pub const fn boss_size(id: crate::components::boss::BossId) -> f32 {
    match id {
        crate::components::boss::BossId::BossA => BOSS_A_SIZE,
        crate::components::boss::BossId::BossB => BOSS_B_SIZE,
        // The dual halves are spawned per-role; the shared pool is 900.
        crate::components::boss::BossId::Dual => BOSS_A_SIZE,
    }
}

// --- Boss brain ---

/// Seconds a boss idles between finishing one pattern and starting the next.
pub const BOSS_PATTERN_COOLDOWN: f32 = 1.1;
pub const BOSS_PATTERN_COOLDOWN_P2: f32 = 0.65;
/// How many recent patterns are excluded from the next random pick.
pub const PATTERN_HISTORY: usize = 2;

// --- Boss A: Tailings Surge ---

pub const SURGE_WINDUP: f32 = 0.75;
pub const SURGE_SPEED: f32 = 260.0;
pub const SURGE_DURATION: f32 = 1.0;
pub const SURGE_TRAIL_WIDTH: f32 = 34.0;
pub const SURGE_TRAIL_WIDTH_P2: f32 = 52.0;
pub const SURGE_TRAIL_LIFE: f32 = 3.0;
/// Damage per hazard tick while standing in tailings.
pub const SURGE_TRAIL_DAMAGE: f32 = 8.0;
pub const SURGE_CHARGES_P2: u32 = 2;
/// How far the phase-2 charge path may curve from the aim direction.
pub const SURGE_CURVE_DEG: f32 = 28.0;

// --- Boss A: Excavator Slam ---

pub const SLAM_WINDUP: f32 = 1.15;
pub const SLAM_RADIUS: f32 = 64.0;
pub const SLAM_RADIUS_P2: f32 = 84.0;
pub const SLAM_DAMAGE: f32 = 35.0;
pub const SLAM_ACID_POOLS_P2: u32 = 3;
pub const ACID_POOL_RADIUS: f32 = 42.0;
pub const ACID_POOL_LIFE: f32 = 5.0;
pub const ACID_POOL_DAMAGE: f32 = 12.0;
pub const SLAM_DEBRIS_P2: u32 = 5;

// --- Boss A: Debris Rain ---

pub const DEBRIS_COUNT: u32 = 6;
pub const DEBRIS_COUNT_P2: u32 = 9;
pub const DEBRIS_FALL_TIME: f32 = 1.1;
pub const DEBRIS_SHADOW_RADIUS: f32 = 24.0;
pub const DEBRIS_IMPACT_RADIUS: f32 = 30.0;
pub const DEBRIS_DAMAGE: f32 = 25.0;
pub const DEBRIS_SPREAD: f32 = 220.0;

// --- Boss B: Mirror Step ---

pub const BLINK_RANGE: f32 = 190.0;
pub const BLINK_TELL: f32 = 0.5;
pub const DECOY_COUNT_P2: u32 = 3;
pub const DECOY_SPLASH_RADIUS: f32 = 36.0;
pub const DECOY_SPLASH_DAMAGE: f32 = 15.0;
pub const DECOY_SIZE: f32 = 48.0;

// --- Boss B: Quicksilver Wave ---

pub const WAVE_WINDUP: f32 = 0.6;
pub const WAVE_SPEED: f32 = 170.0;
pub const WAVE_SPEED_P2: f32 = 240.0;
pub const WAVE_LENGTH: f32 = 150.0;
pub const WAVE_WIDTH: f32 = 44.0;
pub const WAVE_DAMAGE: f32 = 22.0;
pub const WAVE_LIFE: f32 = 4.0;
/// Mercury left behind by a phase-2 wave.
pub const MERCURY_POOL_LIFE: f32 = 4.0;
pub const MERCURY_POOL_RADIUS: f32 = 40.0;
pub const MERCURY_POOL_DAMAGE: f32 = 10.0;
/// Movement multiplier while standing on a mercury surface.
pub const MERCURY_SLIP: f32 = 1.6;

// --- Boss B: Madness Spray ---

pub const SPRAY_WINDUP: f32 = 0.65;
pub const SPRAY_CONE_DEG: f32 = 60.0;
pub const SPRAY_RANGE: f32 = 190.0;
pub const SPRAY_DROPLETS: u32 = 12;
pub const SPRAY_DROPLETS_P2: u32 = 20;
pub const SPRAY_DAMAGE: f32 = 15.0;
pub const SPRAY_SPEED: f32 = 150.0;
pub const SPRAY_LIFE: f32 = 1.6;
pub const REVERSAL_DURATION: f32 = 2.0;
pub const REVERSAL_DURATION_P2: f32 = 4.0;
pub const WISP_SPEED: f32 = 95.0;
pub const WISP_TURN_RATE: f32 = 2.5;
pub const WISP_LIFE: f32 = 4.0;

// --- Hazards and status ---

/// Minimum seconds between hazard damage applications to the player.
pub const HAZARD_TICK: f32 = 0.35;
/// How long a slow stays applied after leaving a slowing hazard.
pub const SLOW_DURATION: f32 = 1.5;
/// Multiplier applied to movement speed while slowed.
pub const SLOW_MULTIPLIER: f32 = 0.6;

// --- Dual boss: Amalgamation ---

pub const AMALGAMATION_INTERVAL_P1: f32 = 30.0;
pub const AMALGAMATION_INTERVAL_P2: f32 = 20.0;
pub const AMALGAMATION_CHANNEL: f32 = 5.0;
pub const AMALGAMATION_RADIUS: f32 = 320.0;
pub const AMALGAMATION_DAMAGE: f32 = 50.0;
/// The dual bosses launch a coordinated combo on this cadence.
pub const DUAL_COMBO_INTERVAL: f32 = 6.4;

// --- Lock-on ---

/// Reticle ring drawn around the locked target.
pub const RETICLE_SIZE: f32 = 56.0;
pub const RETICLE_Z: f32 = 4.0;
/// Off-screen indicator arrow.
pub const TARGET_ARROW_SIZE: f32 = 22.0;
pub const TARGET_ARROW_DISTANCE: f32 = 120.0;
pub const TARGET_ARROW_Z: f32 = 5.0;

// --- Contact ---

/// Knockback applied to the player when a hit lands.
pub const KNOCKBACK_FORCE: f32 = 150.0;
/// Knockback bleeds off exponentially at this rate per second.
pub const KNOCKBACK_DECAY_RATE: f32 = 8.0;
/// Invulnerability after the player is hit.
pub const IFRAME_DURATION: f32 = 0.5;
/// Every hit lands for at least this much, no matter the armor.
pub const MIN_DAMAGE: f32 = 1.0;

// --- Hit feedback ---

/// How long an impact spark stays on screen.
pub const HIT_SPARK_DURATION: f32 = 0.09;
/// Size of the yellow spark fringe.
pub const HIT_SPARK_FRINGE_SIZE: f32 = 20.0;
/// Size of the red spark core.
pub const HIT_SPARK_CORE_SIZE: f32 = 10.0;
/// Sparks draw above every world sprite.
pub const HIT_SPARK_Z: f32 = 3.0;
