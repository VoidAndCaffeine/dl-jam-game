use crate::components::boss::{BossId, DualRole};
use crate::components::player_sprite::{FRAME_COUNT, Facing8, locomotion_dir};
use crate::constants::{BOSS_DEATH_FRAME_SECONDS, BOSS_FRAME_SECONDS, MERCURIL_DEATH_FRAMES};
use bevy::prelude::*;

/// Which full-body sprite pack a boss wears.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BossPack {
    Excavator,
    Mercuril,
}

impl BossPack {
    /// The folder name of this pack inside `assets/sprite_packs`.
    pub fn folder(self) -> &'static str {
        match self {
            BossPack::Excavator => "Excavator-spritesheet",
            BossPack::Mercuril => "Mercuril-spritesheet",
        }
    }

    /// The pack for a single boss id. `Dual` defaults to the Excavator and is
    /// overridden per half via [`BossPack::from_role`].
    pub fn from_id(id: BossId) -> Self {
        match id {
            BossId::BossA | BossId::Dual => BossPack::Excavator,
            BossId::BossB => BossPack::Mercuril,
        }
    }

    pub fn from_role(role: DualRole) -> Self {
        match role {
            DualRole::Excavator => BossPack::Excavator,
            DualRole::Quicksilver => BossPack::Mercuril,
        }
    }

    /// The folder word for this pack's defeat clip.
    pub fn death_word(self) -> &'static str {
        match self {
            BossPack::Excavator => "Tip over",
            BossPack::Mercuril => "turn into goo",
        }
    }

    /// The world size this pack is drawn at.
    pub fn sprite_size(self) -> f32 {
        match self {
            BossPack::Excavator => crate::constants::BOSS_A_SIZE,
            BossPack::Mercuril => crate::constants::BOSS_B_SIZE,
        }
    }
}

/// How long a pack's defeat clip should run before the defeat is announced.
pub fn death_duration(pack: BossPack) -> f32 {
    let frames = BossAnimState::Death.max_frames(pack);
    frames as f32 * BOSS_DEATH_FRAME_SECONDS + crate::constants::BOSS_DEATH_HOLD
}

/// One animated clip a boss can show.
#[derive(Reflect, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum BossAnimState {
    #[default]
    Idle,
    Walk,
    TailingsSurge,
    ExcavatorSlam,
    DebrisRain,
    MirrorStep,
    QuicksilverWave,
    Madness,
    Death,
}

impl BossAnimState {
    /// Idle and Walk cycle; every other clip plays once.
    pub fn loops(self) -> bool {
        matches!(self, BossAnimState::Idle | BossAnimState::Walk)
    }

    /// Whether this clip is an attack (not locomotion or death).
    pub fn is_attack(self) -> bool {
        !matches!(
            self,
            BossAnimState::Idle | BossAnimState::Walk | BossAnimState::Death
        )
    }

    /// Seconds per frame for this clip.
    pub fn frame_seconds(self) -> f32 {
        match self {
            BossAnimState::Death => BOSS_DEATH_FRAME_SECONDS,
            _ => BOSS_FRAME_SECONDS,
        }
    }

    /// How many frames of this clip are actually drawn.
    ///
    /// Every sheet is a 5x5 grid except the Mercuril's death, whose art only
    /// fills the first 12 frames.
    pub fn max_frames(self, pack: BossPack) -> usize {
        match (self, pack) {
            (BossAnimState::Death, BossPack::Mercuril) => MERCURIL_DEATH_FRAMES,
            _ => FRAME_COUNT,
        }
    }

    /// The command-word used by the attack and death folders.
    fn word(self, pack: BossPack) -> &'static str {
        match self {
            BossAnimState::TailingsSurge => "Tailings Surge (Charge)",
            BossAnimState::ExcavatorSlam => "Excavator Slam",
            BossAnimState::DebrisRain => "Debris Rain",
            BossAnimState::MirrorStep => "MirrorStep",
            BossAnimState::QuicksilverWave => "Quicksilver wave",
            BossAnimState::Madness => "Madness",
            BossAnimState::Death => pack.death_word(),
            // Locomotion is handled by `directory`.
            BossAnimState::Idle | BossAnimState::Walk => "",
        }
    }

    /// The folder name for this clip in `pack`, facing `facing`.
    pub fn directory(self, pack: BossPack, facing: Facing8) -> String {
        match self {
            BossAnimState::Idle => locomotion_dir("Idle", "idle", facing),
            BossAnimState::Walk => locomotion_dir("Walk", "walk", facing),
            other => format!("{} {}", other.word(pack), facing.compass()),
        }
    }

    /// The full asset path of this clip's spritesheet.
    pub fn sheet_path(self, pack: BossPack, facing: Facing8) -> String {
        format!(
            "sprite_packs/{}/{}/spritesheet.png",
            pack.folder(),
            self.directory(pack, facing)
        )
    }
}

/// The three frame windows (0-based, inclusive) of a 25-frame attack clip.
#[derive(Reflect, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClipWindows {
    pub windup: (usize, usize),
    pub active: (usize, usize),
    pub recovery: (usize, usize),
}

/// A committed attack's playback plan: which frames belong to which phase, and
/// how long each phase lasts. The frame shown is derived from elapsed time, so
/// an active window loops (the charge) while the rest plays through.
#[derive(Reflect, Debug, Clone, Copy, Default)]
pub struct ClipPlayback {
    pub windows: ClipWindows,
    pub windup: f32,
    pub active: f32,
    pub recovery: f32,
    pub elapsed: f32,
}

impl ClipPlayback {
    pub fn new(windows: ClipWindows, windup: f32, active: f32, recovery: f32) -> Self {
        Self {
            windows,
            windup,
            active,
            recovery,
            elapsed: 0.0,
        }
    }

    pub fn total(&self) -> f32 {
        self.windup + self.active + self.recovery
    }

    /// The frame index to show after `elapsed` seconds of the clip.
    pub fn frame_at(&self, elapsed: f32) -> usize {
        if elapsed < self.windup {
            return window_frame(self.windows.windup, elapsed, self.windup, false);
        }
        let after_windup = elapsed - self.windup;
        if after_windup < self.active {
            return window_frame(self.windows.active, after_windup, self.active, true);
        }
        let after_active = after_windup - self.active;
        window_frame(self.windows.recovery, after_active, self.recovery, false)
    }
}

/// The frame within a window after `t` seconds.
///
/// Looping windows step at the native frame rate so the pose reads as a held
/// cycle; one-shot windows map the window proportionally so a fast-forwarded
/// phase (phase 2 recovery) simply plays its frames faster.
fn window_frame((first, last): (usize, usize), t: f32, duration: f32, loops: bool) -> usize {
    if last <= first {
        return first;
    }
    let count = last - first + 1;
    if duration <= 0.0 {
        return last;
    }
    if loops {
        let steps = (t / BOSS_FRAME_SECONDS).floor().max(0.0) as usize;
        first + steps % count
    } else {
        let progress = (t / duration).clamp(0.0, 1.0);
        first + ((progress * count as f32).floor() as usize).min(count - 1)
    }
}

/// Drives a boss's animated sprite. Lives on the boss entity.
#[derive(Component, Reflect, Debug)]
pub struct BossAnimation {
    pub pack: BossPack,
    pub state: BossAnimState,
    pub facing: Facing8,
    /// Current frame index into the 5x5 sheet.
    pub frame: usize,
    /// Drives frame advancement.
    pub frame_timer: Timer,
    /// Seconds left of a one-shot attack before locomotion resumes.
    pub action_remaining: f32,
    /// Last frame's world position, used to tell Idle from Walk.
    pub last_position: Vec2,
    /// Seconds the boss has been still, gating the Walk -> Idle drop.
    pub still_time: f32,
    /// Timeline of the running attack, if any.
    pub clip: Option<ClipPlayback>,
}

impl BossAnimation {
    pub fn new(pack: BossPack, position: Vec2) -> Self {
        Self {
            pack,
            state: BossAnimState::Idle,
            facing: Facing8::Down,
            frame: 0,
            frame_timer: Timer::from_seconds(BOSS_FRAME_SECONDS, TimerMode::Repeating),
            action_remaining: 0.0,
            last_position: position,
            still_time: 0.0,
            clip: None,
        }
    }

    /// Switches to `state` from its first frame, re-timing the frame clock.
    pub fn set_state(&mut self, state: BossAnimState) {
        if self.state == state {
            return;
        }
        self.state = state;
        self.frame = 0;
        self.frame_timer = Timer::from_seconds(state.frame_seconds(), TimerMode::Repeating);
        self.clip = None;
    }

    /// Starts an attack clip with its phase windows and timings.
    pub fn set_attack(&mut self, state: BossAnimState, facing: Facing8, clip: ClipPlayback) {
        self.set_state(state);
        self.facing = facing;
        self.frame = 0;
        self.action_remaining = clip.total();
        self.clip = Some(clip);
    }

    pub fn is_acting(&self) -> bool {
        self.action_remaining > 0.0
    }

    /// True while an attack clip is still playing its windup frames.
    ///
    /// Used so a boss may lunge in during the tell, then plant while the hit
    /// resolves.
    pub fn is_winding_up(&self) -> bool {
        self.clip
            .as_ref()
            .is_some_and(|clip| clip.elapsed < clip.windup)
    }

    /// Advances the clip, or the plain frame clock for locomotion and death.
    pub fn advance_frame(&mut self, dt: f32) {
        if let Some(clip) = &mut self.clip {
            clip.elapsed += dt;
            self.frame = clip.frame_at(clip.elapsed);
            return;
        }
        self.frame_timer
            .tick(core::time::Duration::from_secs_f32(dt));
        if !self.frame_timer.just_finished() {
            return;
        }
        let max = self.state.max_frames(self.pack);
        if self.state.loops() {
            self.frame = (self.frame + 1) % max;
        } else if self.frame + 1 < max {
            self.frame += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn death_clips_use_the_drawn_frame_count() {
        assert_eq!(
            BossAnimState::Death.max_frames(BossPack::Mercuril),
            MERCURIL_DEATH_FRAMES
        );
        assert_eq!(
            BossAnimState::Death.max_frames(BossPack::Excavator),
            FRAME_COUNT
        );
        assert_eq!(
            BossAnimState::Idle.max_frames(BossPack::Mercuril),
            FRAME_COUNT
        );
    }

    #[test]
    fn boss_folders_match_the_generated_packs() {
        assert_eq!(
            BossAnimState::TailingsSurge.sheet_path(BossPack::Excavator, Facing8::Right),
            "sprite_packs/Excavator-spritesheet/Tailings Surge (Charge) Right/spritesheet.png"
        );
        assert_eq!(
            BossAnimState::ExcavatorSlam.sheet_path(BossPack::Excavator, Facing8::Up),
            "sprite_packs/Excavator-spritesheet/Excavator Slam Up/spritesheet.png"
        );
        assert_eq!(
            BossAnimState::Death.sheet_path(BossPack::Excavator, Facing8::Down),
            "sprite_packs/Excavator-spritesheet/Tip over Down/spritesheet.png"
        );
        assert_eq!(
            BossAnimState::Death.sheet_path(BossPack::Mercuril, Facing8::Down),
            "sprite_packs/Mercuril-spritesheet/turn into goo Down/spritesheet.png"
        );
        assert_eq!(
            BossAnimState::MirrorStep.sheet_path(BossPack::Mercuril, Facing8::UpRight),
            "sprite_packs/Mercuril-spritesheet/MirrorStep Northeast/spritesheet.png"
        );
        assert_eq!(
            BossAnimState::QuicksilverWave.sheet_path(BossPack::Mercuril, Facing8::Left),
            "sprite_packs/Mercuril-spritesheet/Quicksilver wave Left/spritesheet.png"
        );
        assert_eq!(
            BossAnimState::Madness.sheet_path(BossPack::Mercuril, Facing8::Right),
            "sprite_packs/Mercuril-spritesheet/Madness Right/spritesheet.png"
        );
    }

    #[test]
    fn boss_idle_uses_the_iso_locomotion_layout() {
        assert_eq!(
            BossAnimState::Idle.sheet_path(BossPack::Excavator, Facing8::Left),
            "sprite_packs/Excavator-spritesheet/Idle Left/spritesheet.png"
        );
        assert_eq!(
            BossAnimState::Walk.sheet_path(BossPack::Mercuril, Facing8::Down),
            "sprite_packs/Mercuril-spritesheet/iso_walk_down_right/spritesheet.png"
        );
    }

    #[test]
    fn a_death_clip_clamps_at_its_last_drawn_frame() {
        let mut anim = BossAnimation::new(BossPack::Mercuril, Vec2::ZERO);
        anim.set_state(BossAnimState::Death);
        for _ in 0..100 {
            anim.advance_frame(BOSS_DEATH_FRAME_SECONDS + 0.001);
        }
        assert_eq!(anim.frame, MERCURIL_DEATH_FRAMES - 1);
    }

    #[test]
    fn a_looping_clip_wraps() {
        let mut anim = BossAnimation::new(BossPack::Excavator, Vec2::ZERO);
        for _ in 0..FRAME_COUNT {
            anim.advance_frame(BOSS_FRAME_SECONDS + 0.001);
        }
        assert_eq!(anim.frame, 0);
    }
}
