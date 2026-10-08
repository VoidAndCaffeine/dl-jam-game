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

/// How long the player is rooted and the swing graphic is drawn.
pub const ATTACK_DURATION: f32 = 0.2;

// --- Bosses ---

pub const BOSS_A_HEALTH: f32 = 1012.5;
pub const BOSS_B_HEALTH: f32 = 500.0;
/// The dual boss fights from a single shared pool.
pub const DUAL_BOSS_HEALTH: f32 = 1800.0;

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

/// The world size the madness-spray and wisp droplets are drawn at. Both use
/// the same art scale, so one constant covers the pair.
pub const WISP_SPRITE_SIZE: f32 = 16.0;

// --- Effect layering ---

/// Z of an effect that lies on the floor (caustic pools, trails, impact
/// bursts). Below the boss so it never covers the sprite standing over it.
pub const GROUND_EFFECT_Z: f32 = -0.5;
/// Z of an airborne effect (waves, droplets, decoys). Above the boss so it
/// reads as passing over the arena.
pub const FLOATING_EFFECT_Z: f32 = 2.0;
/// Airborne effects are drawn this much larger than their hitbox so the small
/// spray droplets stay legible. Purely cosmetic.
pub const SPRAY_GRAPHIC_SCALE: f32 = 2.5;

/// Damage hitboxes are shrunk to this fraction of the drawn effect.
///
/// The art is a circle padded inside a square quad, so the visible shape is
/// smaller than the sprite; matching the hitbox to the visible shape makes
/// collisions feel as fair as they look.
pub const HITBOX_SHRINK: f32 = 0.9;

/// The sprite size of a boss id. Both dual halves keep their native sizes.
pub const fn boss_size(id: crate::components::boss::BossId) -> f32 {
    match id {
        crate::components::boss::BossId::BossA => BOSS_A_SIZE,
        crate::components::boss::BossId::BossB => BOSS_B_SIZE,
        // The dual halves are spawned per-role; the shared pool is 900.
        crate::components::boss::BossId::Dual => BOSS_A_SIZE,
    }
}

// --- Boss animation ---

/// Seconds per frame for a boss clip (~2.33s for 25 frames, like the player).
pub const BOSS_FRAME_SECONDS: f32 = 0.093;
/// Boss death clips run a little faster so a defeat does not drag.
pub const BOSS_DEATH_FRAME_SECONDS: f32 = 0.06;
/// Extra time the final death frame is held before the defeat fires.
pub const BOSS_DEATH_HOLD: f32 = 0.4;
/// The Mercuril's death sheet only has its first 12 frames drawn.
pub const MERCURIL_DEATH_FRAMES: usize = 12;
/// How far a boss must move in a frame to count as walking.
pub const BOSS_WALK_THRESHOLD: f32 = 0.6;
/// How long a boss must be still before it drops from Walk to Idle.
///
/// This is hysteresis: without it, a boss hovering on the walk threshold
/// restarts its Walk clip every few frames and visibly flickers.
pub const BOSS_STILL_GRACE: f32 = 0.18;
/// Every boss attack sheet is a 25-frame clip.
pub const BOSS_CLIP_FRAMES: usize = 25;
/// Seconds to play a whole attack clip at 1x.
pub const BOSS_CLIP_SECONDS: f32 = BOSS_FRAME_SECONDS * BOSS_CLIP_FRAMES as f32;

/// When frame `n` (1-based) of a clip has finished playing.
///
/// Attack phase timings are expressed against these boundaries so the gameplay
/// stays in lockstep with the art.
pub const fn boss_frame(n: u32) -> f32 {
    BOSS_FRAME_SECONDS * n as f32
}

// --- Boss brain ---

/// Seconds a boss idles between finishing one pattern and starting the next.
///
/// The attack clip already owns its own recovery frames, so this is only the
/// extra pause; keeping it short is what makes the boss feel aggressive.
pub const BOSS_PATTERN_COOLDOWN: f32 = 0.2;
pub const BOSS_PATTERN_COOLDOWN_P2: f32 = 0.1;
/// Recovery/cooldown frames play this much faster, which tightens the gap
/// between attacks without touching the windup tell. Phase 2 trims harder.
pub const BOSS_P1_RECOVERY_SPEED: f32 = 1.7;
pub const BOSS_P2_RECOVERY_SPEED: f32 = 2.5;
/// How many recent patterns are excluded from the next random pick.
pub const PATTERN_HISTORY: usize = 2;

/// Seconds a boss holds off before its first attack of the fight, so the player
/// can get into position and get their bearings. Tuneable; keep within 0.8-2.0s.
pub const BOSS_OPENING_GRACE: f32 = 1.5;

// --- Boss movement ---

/// Speed a boss walks while repositioning.
pub const BOSS_MOVE_SPEED: f32 = 95.0;
/// Single bosses close to within this range of the player, then hold.
pub const BOSS_STANDOFF_DISTANCE: f32 = 105.0;
/// Hysteresis band around a single boss's standoff so it does not twitch in
/// and out of the Walk clip as the player shuffles.
pub const BOSS_MOVE_DEAD_ZONE: f32 = 12.0;
/// A dual half on its turn closes to this range of the player.
pub const BOSS_DUAL_APPROACH_DISTANCE: f32 = 96.0;
/// A dual half not on its turn backs out to this range.
pub const BOSS_DUAL_RETREAT_DISTANCE: f32 = 260.0;
/// Wide hysteresis band for the dual halves: between the approach and retreat
/// ranges both hold station instead of endlessly chasing or fleeing.
pub const BOSS_DUAL_DEAD_ZONE: f32 = 20.0;
/// Seconds of player movement a boss leads when aiming a projectile.
pub const BOSS_AIM_LEAD: f32 = 0.25;

// --- Boss A: Tailings Surge ---

/// Frame 9: the charge begins.
pub const SURGE_WINDUP: f32 = boss_frame(9);
/// Frames 13-25: recovery after the charge.
pub const SURGE_RECOVERY: f32 = boss_frame(13);
/// The pause between the two phase-2 charges.
pub const SURGE_REWINDUP: f32 = 0.35;
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

/// Frame 7: the slam fires; frames 8-12 stay live.
pub const SLAM_WINDUP: f32 = boss_frame(7);
/// How long the quake stays live. Stretched past its authored frames so the
/// impact flare has time to read while the hitbox is up.
pub const SLAM_ACTIVE: f32 = 0.9;
/// Frames 13-25: recovery.
pub const SLAM_RECOVERY: f32 = boss_frame(13);
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
/// Frame 7: the shadows have fallen, impacts land during frames 8-11.
pub const DEBRIS_WINDUP: f32 = boss_frame(7);
pub const DEBRIS_ACTIVE: f32 = boss_frame(4);
/// Frames 12-25: recovery.
pub const DEBRIS_RECOVERY: f32 = boss_frame(14);
pub const DEBRIS_SHADOW_RADIUS: f32 = 24.0;
pub const DEBRIS_IMPACT_RADIUS: f32 = 30.0;
pub const DEBRIS_DAMAGE: f32 = 25.0;
pub const DEBRIS_SPREAD: f32 = 220.0;
/// How long the dust cloud lingers after a chunk lands. The landing damage is
/// one-shot, so this is visual only.
pub const DEBRIS_DUST_LIFE: f32 = 1.4;

// --- Boss B: Mirror Step ---

pub const BLINK_RANGE: f32 = 190.0;
/// Frame 8: the teleport begins; frames 9-14 are the vanish.
pub const BLINK_WINDUP: f32 = boss_frame(8);
pub const BLINK_VANISH: f32 = boss_frame(6);
/// Frames 15-25: recovery.
pub const BLINK_RECOVERY: f32 = boss_frame(11);
pub const DECOY_COUNT_P2: u32 = 3;
pub const DECOY_SPLASH_RADIUS: f32 = 36.0;
pub const DECOY_SPLASH_DAMAGE: f32 = 15.0;
pub const DECOY_SIZE: f32 = 48.0;

// --- Boss B: Quicksilver Wave ---

/// Frame 9: the wave launches on frame 10.
pub const WAVE_WINDUP: f32 = boss_frame(9);
pub const WAVE_ACTIVE: f32 = boss_frame(1);
/// Frames 11-25: recovery.
pub const WAVE_RECOVERY: f32 = boss_frame(15);
pub const WAVE_SPEED: f32 = 145.0;
pub const WAVE_SPEED_P2: f32 = 205.0;
pub const WAVE_LENGTH: f32 = 150.0;
/// The wall's depth along its travel. Matches the trimmed art's aspect so the
/// sheet is not stretched.
pub const WAVE_WIDTH: f32 = 47.0;
pub const WAVE_DAMAGE: f32 = 22.0;
pub const WAVE_LIFE: f32 = 4.0;
/// Mercury left behind by a phase-2 wave.
pub const MERCURY_POOL_LIFE: f32 = 4.0;
pub const MERCURY_POOL_RADIUS: f32 = 40.0;
pub const MERCURY_POOL_DAMAGE: f32 = 10.0;
/// Movement multiplier while standing on a mercury surface.
pub const MERCURY_SLIP: f32 = 1.6;

// --- Boss B: Madness Spray ---

/// Frame 7: the spray releases.
pub const SPRAY_WINDUP: f32 = boss_frame(7);
pub const SPRAY_ACTIVE: f32 = boss_frame(5);
/// Frames 13-25: recovery.
pub const SPRAY_RECOVERY: f32 = boss_frame(13);
pub const SPRAY_CONE_DEG: f32 = 60.0;
pub const SPRAY_RANGE: f32 = 190.0;
pub const SPRAY_DROPLETS: u32 = 12;
pub const SPRAY_DROPLETS_P2: u32 = 20;
pub const SPRAY_DAMAGE: f32 = 15.0;
pub const SPRAY_SPEED: f32 = 130.0;
pub const SPRAY_LIFE: f32 = 1.6;
pub const REVERSAL_DURATION: f32 = 2.0;
pub const REVERSAL_DURATION_P2: f32 = 4.0;
pub const WISP_SPEED: f32 = 80.0;
pub const WISP_TURN_RATE: f32 = 2.5;
pub const WISP_LIFE: f32 = 4.0;

// --- Hazards and status ---

/// Minimum seconds between hazard damage applications to the player.
pub const HAZARD_TICK: f32 = 0.35;
/// The fraction of full opacity a lingering hazard never fades below, so the
/// art stays legible right up to despawn.
pub const HAZARD_FADE_FRACTION: f32 = 0.25;
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
/// Fallback cadence for a dual turn when no attack was chosen.
pub const DUAL_COMBO_INTERVAL: f32 = 2.5;
/// Breathing room after a dual attack's damage window before the next turn.
/// The next half may wind up during the previous recovery, so the fight keeps
/// moving without ever stacking two damage windows.
pub const DUAL_COMBO_GAP: f32 = 0.35;

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
