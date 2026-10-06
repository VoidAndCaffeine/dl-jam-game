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
