use crate::components::gear::GearSet;
use crate::levels::LevelId;
use bevy::prelude::*;

/// Every sprite sheet in `assets/sprite_packs` is a 5x5 grid of frames.
pub const FRAME_COLUMNS: u32 = 5;
pub const FRAME_ROWS: u32 = 5;
pub const FRAME_COUNT: usize = (FRAME_COLUMNS * FRAME_ROWS) as usize;
/// Width/height of one frame in the source sheets, in pixels.
pub const FRAME_SIZE: u32 = 256;
/// Seconds each frame is shown (the source clips run ~2.33s for 25 frames).
pub const FRAME_SECONDS: f32 = 0.093;
/// Size the character is drawn at in world space.
pub const PLAYER_SPRITE_SIZE: f32 = 64.0;

/// Which full-body sprite pack the player wears.
///
/// This is deliberately separate from the gameplay [`GearSet`]: the farm look
/// is not craftable gear, and only it has the planting and watering clips.
#[derive(Reflect, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PlayerLook {
    #[default]
    Farmer,
    Starter,
    BossA,
    BossB,
    Master,
}

impl PlayerLook {
    pub const ALL: [PlayerLook; 5] = [
        PlayerLook::Farmer,
        PlayerLook::Starter,
        PlayerLook::BossA,
        PlayerLook::BossB,
        PlayerLook::Master,
    ];

    /// The folder name of this look inside `assets/sprite_packs`.
    pub fn folder(self) -> &'static str {
        match self {
            PlayerLook::Farmer => "Farmer-spritesheet",
            PlayerLook::Starter => "Starter-Gear-spritesheet",
            PlayerLook::BossA => "Ember-Gear-spritesheet",
            PlayerLook::BossB => "Gloom-robe-spritesheet",
            PlayerLook::Master => "Layered-Aegis-spritesheet",
        }
    }

    /// The look for the farm's armor set.
    pub fn from_gear(set: GearSet) -> Self {
        match set {
            GearSet::Starter => PlayerLook::Starter,
            GearSet::BossA => PlayerLook::BossA,
            GearSet::BossB => PlayerLook::BossB,
            GearSet::Master => PlayerLook::Master,
        }
    }

    /// The look the player wears in `level` with `armor` equipped.
    ///
    /// The farm always shows the farmer because only that pack has the
    /// planting and watering clips; arenas show the equipped armor set, falling
    /// back to the farmer when nothing is equipped.
    pub fn for_context(level: LevelId, armor: Option<GearSet>) -> Self {
        if level == LevelId::Farm {
            return PlayerLook::Farmer;
        }
        armor.map_or(PlayerLook::Farmer, PlayerLook::from_gear)
    }
}

/// The eight facing directions the sprite art is drawn for.
#[derive(Reflect, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Facing8 {
    #[default]
    Down,
    DownRight,
    Right,
    UpRight,
    Up,
    UpLeft,
    Left,
    DownLeft,
}

impl Facing8 {
    pub const ALL: [Facing8; 8] = [
        Facing8::Down,
        Facing8::DownRight,
        Facing8::Right,
        Facing8::UpRight,
        Facing8::Up,
        Facing8::UpLeft,
        Facing8::Left,
        Facing8::DownLeft,
    ];

    /// The direction a non-zero vector points in, if it points anywhere.
    ///
    /// World space is y-up, so `+y` is up and `+x` is right.
    pub fn from_direction(direction: Vec2) -> Option<Self> {
        if direction == Vec2::ZERO {
            return None;
        }
        let angle = direction.y.atan2(direction.x).to_degrees();
        Some(match angle {
            a if (-22.5..22.5).contains(&a) => Facing8::Right,
            a if (22.5..67.5).contains(&a) => Facing8::UpRight,
            a if (67.5..112.5).contains(&a) => Facing8::Up,
            a if (112.5..157.5).contains(&a) => Facing8::UpLeft,
            a if !(-157.5..157.5).contains(&a) => Facing8::Left,
            a if (-157.5..-112.5).contains(&a) => Facing8::DownLeft,
            a if (-112.5..-67.5).contains(&a) => Facing8::Down,
            _ => Facing8::DownRight,
        })
    }

    /// The compass word used by the attack, death and farm-action folders.
    pub fn compass(self) -> &'static str {
        match self {
            Facing8::Down => "Down",
            Facing8::DownRight => "Southeast",
            Facing8::Right => "Right",
            Facing8::UpRight => "Northeast",
            Facing8::Up => "Up",
            Facing8::UpLeft => "Northwest",
            Facing8::Left => "Left",
            Facing8::DownLeft => "Southwest",
        }
    }
}

/// One clip in a sprite pack.
#[derive(Reflect, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PlayerAnimState {
    #[default]
    Idle,
    Walk,
    LightAttack,
    HeavyAttack,
    Death,
    Water,
    Plant,
}

impl PlayerAnimState {
    /// Idle and Walk cycle; every other clip plays once and then either holds
    /// (Death) or hands control back to locomotion.
    pub fn loops(self) -> bool {
        matches!(self, PlayerAnimState::Idle | PlayerAnimState::Walk)
    }

    /// Seconds per frame for this clip. The farm actions are long in the source
    /// art, so they run 1.5x faster to stay snappy.
    pub fn frame_seconds(self) -> f32 {
        match self {
            PlayerAnimState::Water | PlayerAnimState::Plant => FRAME_SECONDS / 1.5,
            _ => FRAME_SECONDS,
        }
    }

    /// The folder name for this clip in `look`, facing `facing`.
    pub fn directory(self, look: PlayerLook, facing: Facing8) -> String {
        match self {
            PlayerAnimState::Idle => locomotion_dir("Idle", "idle", facing),
            PlayerAnimState::Walk => locomotion_dir("Walk", "walk", facing),
            PlayerAnimState::LightAttack => {
                format!("{} {}", light_attack_word(look), facing.compass())
            }
            PlayerAnimState::HeavyAttack => {
                format!("{} {}", heavy_attack_word(look), facing.compass())
            }
            PlayerAnimState::Death => format!("Death {}", facing.compass()),
            PlayerAnimState::Water => format!("water {}", facing.compass()),
            PlayerAnimState::Plant => format!("plant {}", facing.compass()),
        }
    }

    /// The full asset path of this clip's spritesheet.
    pub fn sheet_path(self, look: PlayerLook, facing: Facing8) -> String {
        format!(
            "sprite_packs/{}/{}/spritesheet.png",
            look.folder(),
            self.directory(look, facing)
        )
    }
}

/// Idle and Walk only exist as plain folders for the three "left" directions;
/// the remaining five were exported with an `iso_` prefix.
///
/// Shared with the boss packs, which follow the exact same export layout.
pub(crate) fn locomotion_dir(title: &str, lower: &str, facing: Facing8) -> String {
    match facing {
        Facing8::Left => format!("{title} Left"),
        Facing8::UpLeft => format!("{title} Northwest"),
        Facing8::DownLeft => format!("{title} Southwest"),
        other => format!("iso_{lower}_{}", iso_suffix(other)),
    }
}

pub(crate) fn iso_suffix(facing: Facing8) -> &'static str {
    match facing {
        Facing8::Down => "down_right",
        Facing8::DownRight => "southeast_right",
        Facing8::Right => "right_right",
        Facing8::UpRight => "northeast_right",
        Facing8::Up => "up_right",
        Facing8::UpLeft | Facing8::Left | Facing8::DownLeft => "left",
    }
}

/// The light-attack folder casing differs across packs: farmer and starter use
/// `Light Attack`, the boss A/B packs use `light attack`, and the master pack
/// uses `light Attack`.
fn light_attack_word(look: PlayerLook) -> &'static str {
    match look {
        PlayerLook::Farmer | PlayerLook::Starter => "Light Attack",
        PlayerLook::BossA | PlayerLook::BossB => "light attack",
        PlayerLook::Master => "light Attack",
    }
}

/// Only the farmer pack capitalises the `H` in `Heavy`.
fn heavy_attack_word(look: PlayerLook) -> &'static str {
    match look {
        PlayerLook::Farmer => "Heavy attack",
        PlayerLook::Starter | PlayerLook::BossA | PlayerLook::BossB | PlayerLook::Master => {
            "heavy attack"
        }
    }
}

/// Drives the player's animated sprite. Lives on the player entity.
#[derive(Component, Reflect, Debug)]
pub struct PlayerAnimation {
    /// Which sprite pack is currently worn.
    pub look: PlayerLook,
    /// The clip being shown.
    pub state: PlayerAnimState,
    /// The direction the clip is shown for.
    pub facing: Facing8,
    /// Current frame index into the 5x5 sheet.
    pub frame: usize,
    /// Drives frame advancement.
    pub frame_timer: Timer,
    /// A one-shot clip that must finish before locomotion resumes.
    pub action: Option<PlayerAnimState>,
    /// The room the current look was chosen for, so a room change resets the
    /// animation even when the look itself stays the same.
    pub level: LevelId,
}

impl Default for PlayerAnimation {
    fn default() -> Self {
        Self {
            look: PlayerLook::Farmer,
            state: PlayerAnimState::Idle,
            facing: Facing8::Down,
            frame: 0,
            frame_timer: Timer::from_seconds(FRAME_SECONDS, TimerMode::Repeating),
            action: None,
            level: LevelId::Farm,
        }
    }
}

impl PlayerAnimation {
    /// Starts a one-shot clip from its first frame.
    pub fn start_action(&mut self, state: PlayerAnimState) {
        self.action = Some(state);
        self.restart_clip(state);
    }

    /// Returns to the resting pose, used when a room change replaces the look.
    pub fn reset(&mut self) {
        self.action = None;
        self.restart_clip(PlayerAnimState::Idle);
    }

    pub fn is_acting(&self) -> bool {
        self.action.is_some()
    }

    /// Switches to `state` from its first frame, re-timing the frame clock to
    /// that clip's speed.
    fn restart_clip(&mut self, state: PlayerAnimState) {
        self.state = state;
        self.frame = 0;
        self.frame_timer = Timer::from_seconds(state.frame_seconds(), TimerMode::Repeating);
    }
}

/// Advances `anim` by `dt`.
///
/// `movement` is the player's input direction and decides Idle versus Walk;
/// `facing` is the direction to look in (movement input, or the swing/boss
/// direction while attacking), so the player can idle while still facing the
/// boss; `attacking` is the swing being thrown, if any. Pure so it can be tested
/// without an asset server.
pub fn step_animation(
    anim: &mut PlayerAnimation,
    dt: f32,
    movement: Vec2,
    facing: Vec2,
    attacking: Option<crate::components::attack::AttackType>,
) {
    use crate::components::attack::AttackType;

    // The one-shot clip wins until it finishes, then Death holds forever.
    let target = if let Some(action) = anim.action {
        action
    } else if anim.state == PlayerAnimState::Death {
        PlayerAnimState::Death
    } else if let Some(attack) = attacking {
        match attack {
            AttackType::Light => PlayerAnimState::LightAttack,
            AttackType::Heavy => PlayerAnimState::HeavyAttack,
        }
    } else if movement != Vec2::ZERO {
        PlayerAnimState::Walk
    } else {
        PlayerAnimState::Idle
    };

    if target != anim.state {
        anim.restart_clip(target);
    }

    if anim.state != PlayerAnimState::Death
        && let Some(facing) = Facing8::from_direction(facing)
    {
        anim.facing = facing;
    }

    anim.frame_timer
        .tick(core::time::Duration::from_secs_f32(dt));
    if anim.frame_timer.just_finished() {
        if anim.state.loops() {
            anim.frame = (anim.frame + 1) % FRAME_COUNT;
        } else if anim.frame + 1 < FRAME_COUNT {
            anim.frame += 1;
        } else {
            anim.action = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::attack::AttackType;

    #[test]
    fn player_animation_defaults_to_the_farmer_idle() {
        let anim = PlayerAnimation::default();
        assert_eq!(anim.look, PlayerLook::Farmer);
        assert_eq!(anim.state, PlayerAnimState::Idle);
        assert_eq!(anim.facing, Facing8::Down);
        assert_eq!(anim.frame, 0);
        assert!(!anim.is_acting());
    }

    #[test]
    fn the_farm_always_wears_the_farmer() {
        for armor in [
            None,
            Some(GearSet::Starter),
            Some(GearSet::BossA),
            Some(GearSet::BossB),
            Some(GearSet::Master),
        ] {
            assert_eq!(
                PlayerLook::for_context(LevelId::Farm, armor),
                PlayerLook::Farmer
            );
        }
    }

    #[test]
    fn arenas_wear_the_equipped_armor_set() {
        let cases = [
            (GearSet::Starter, PlayerLook::Starter),
            (GearSet::BossA, PlayerLook::BossA),
            (GearSet::BossB, PlayerLook::BossB),
            (GearSet::Master, PlayerLook::Master),
        ];
        for arena in [LevelId::ArenaA, LevelId::ArenaB, LevelId::ArenaDual] {
            for (gear, look) in cases {
                assert_eq!(PlayerLook::for_context(arena, Some(gear)), look);
            }
        }
    }

    #[test]
    fn an_unarmoured_arena_falls_back_to_the_farmer() {
        assert_eq!(
            PlayerLook::for_context(LevelId::ArenaA, None),
            PlayerLook::Farmer
        );
    }

    #[test]
    fn every_look_folder_is_unique() {
        let mut folders: Vec<&str> = PlayerLook::ALL.iter().map(|l| l.folder()).collect();
        folders.sort_unstable();
        folders.dedup();
        assert_eq!(folders.len(), PlayerLook::ALL.len());
    }

    #[test]
    fn facing_maps_the_eight_cardinal_and_diagonal_directions() {
        let cases = [
            (Vec2::new(1.0, 0.0), Facing8::Right),
            (Vec2::new(1.0, 1.0), Facing8::UpRight),
            (Vec2::new(0.0, 1.0), Facing8::Up),
            (Vec2::new(-1.0, 1.0), Facing8::UpLeft),
            (Vec2::new(-1.0, 0.0), Facing8::Left),
            (Vec2::new(-1.0, -1.0), Facing8::DownLeft),
            (Vec2::new(0.0, -1.0), Facing8::Down),
            (Vec2::new(1.0, -1.0), Facing8::DownRight),
        ];
        for (direction, expected) in cases {
            assert_eq!(
                Facing8::from_direction(direction),
                Some(expected),
                "{direction:?}"
            );
        }
    }

    #[test]
    fn a_zero_direction_has_no_facing() {
        assert_eq!(Facing8::from_direction(Vec2::ZERO), None);
    }

    #[test]
    fn every_facing_has_a_distinct_compass_word() {
        let mut words: Vec<&str> = Facing8::ALL.iter().map(|f| f.compass()).collect();
        words.sort_unstable();
        words.dedup();
        assert_eq!(words.len(), Facing8::ALL.len());
    }

    #[test]
    fn farm_locomotion_uses_the_iso_folders_where_the_art_has_them() {
        assert_eq!(
            PlayerAnimState::Idle.sheet_path(PlayerLook::Farmer, Facing8::Down),
            "sprite_packs/Farmer-spritesheet/iso_idle_down_right/spritesheet.png"
        );
        assert_eq!(
            PlayerAnimState::Walk.sheet_path(PlayerLook::Farmer, Facing8::Right),
            "sprite_packs/Farmer-spritesheet/iso_walk_right_right/spritesheet.png"
        );
        assert_eq!(
            PlayerAnimState::Idle.sheet_path(PlayerLook::Farmer, Facing8::Left),
            "sprite_packs/Farmer-spritesheet/Idle Left/spritesheet.png"
        );
        assert_eq!(
            PlayerAnimState::Walk.sheet_path(PlayerLook::Farmer, Facing8::UpLeft),
            "sprite_packs/Farmer-spritesheet/Walk Northwest/spritesheet.png"
        );
        assert_eq!(
            PlayerAnimState::Idle.sheet_path(PlayerLook::Farmer, Facing8::DownLeft),
            "sprite_packs/Farmer-spritesheet/Idle Southwest/spritesheet.png"
        );
    }

    #[test]
    fn farming_actions_point_at_their_folders() {
        assert_eq!(
            PlayerAnimState::Water.sheet_path(PlayerLook::Farmer, Facing8::Up),
            "sprite_packs/Farmer-spritesheet/water Up/spritesheet.png"
        );
        assert_eq!(
            PlayerAnimState::Plant.sheet_path(PlayerLook::Farmer, Facing8::Down),
            "sprite_packs/Farmer-spritesheet/plant Down/spritesheet.png"
        );
    }

    #[test]
    fn attack_folder_casing_matches_each_pack() {
        assert_eq!(
            PlayerAnimState::LightAttack.sheet_path(PlayerLook::Starter, Facing8::Down),
            "sprite_packs/Starter-Gear-spritesheet/Light Attack Down/spritesheet.png"
        );
        assert_eq!(
            PlayerAnimState::LightAttack.sheet_path(PlayerLook::BossA, Facing8::UpRight),
            "sprite_packs/Ember-Gear-spritesheet/light attack Northeast/spritesheet.png"
        );
        assert_eq!(
            PlayerAnimState::LightAttack.sheet_path(PlayerLook::Master, Facing8::Down),
            "sprite_packs/Layered-Aegis-spritesheet/light Attack Down/spritesheet.png"
        );
        assert_eq!(
            PlayerAnimState::HeavyAttack.sheet_path(PlayerLook::Farmer, Facing8::Right),
            "sprite_packs/Farmer-spritesheet/Heavy attack Right/spritesheet.png"
        );
        assert_eq!(
            PlayerAnimState::HeavyAttack.sheet_path(PlayerLook::Master, Facing8::Left),
            "sprite_packs/Layered-Aegis-spritesheet/heavy attack Left/spritesheet.png"
        );
        assert_eq!(
            PlayerAnimState::Death.sheet_path(PlayerLook::BossB, Facing8::DownLeft),
            "sprite_packs/Gloom-robe-spritesheet/Death Southwest/spritesheet.png"
        );
    }

    #[test]
    fn walking_turns_the_face_without_playing_a_one_shot() {
        let mut anim = PlayerAnimation::default();
        step_animation(&mut anim, 0.0, Vec2::X, Vec2::X, None);
        assert_eq!(anim.state, PlayerAnimState::Walk);
        assert_eq!(anim.facing, Facing8::Right);
    }

    #[test]
    fn standing_still_idles_while_still_facing_the_target() {
        let mut anim = PlayerAnimation::default();
        step_animation(&mut anim, 0.0, Vec2::ZERO, Vec2::new(0.0, 1.0), None);
        assert_eq!(anim.state, PlayerAnimState::Idle);
        assert_eq!(anim.facing, Facing8::Up);
    }

    #[test]
    fn standing_still_returns_to_idle_but_keeps_facing() {
        let mut anim = PlayerAnimation::default();
        step_animation(
            &mut anim,
            0.0,
            Vec2::new(0.0, 1.0),
            Vec2::new(0.0, 1.0),
            None,
        );
        let facing = anim.facing;
        step_animation(&mut anim, 0.0, Vec2::ZERO, Vec2::ZERO, None);
        assert_eq!(anim.state, PlayerAnimState::Idle);
        assert_eq!(anim.facing, facing);
    }

    #[test]
    fn attacking_selects_the_matching_clip() {
        let mut anim = PlayerAnimation::default();
        step_animation(&mut anim, 0.0, Vec2::X, Vec2::X, Some(AttackType::Heavy));
        assert_eq!(anim.state, PlayerAnimState::HeavyAttack);
        step_animation(&mut anim, 0.0, Vec2::X, Vec2::X, Some(AttackType::Light));
        assert_eq!(anim.state, PlayerAnimState::LightAttack);
    }

    #[test]
    fn frames_advance_over_a_frame_duration() {
        let mut anim = PlayerAnimation::default();
        step_animation(
            &mut anim,
            FRAME_SECONDS + 0.001,
            Vec2::ZERO,
            Vec2::ZERO,
            None,
        );
        assert_eq!(anim.frame, 1);
    }

    #[test]
    fn farm_action_clips_run_faster_than_the_rest() {
        let water = PlayerAnimState::Water.frame_seconds();
        assert_eq!(water, FRAME_SECONDS / 1.5);
        assert!(water < PlayerAnimState::Idle.frame_seconds());
        assert_eq!(PlayerAnimState::Plant.frame_seconds(), water);
        for state in [
            PlayerAnimState::Idle,
            PlayerAnimState::Walk,
            PlayerAnimState::LightAttack,
            PlayerAnimState::HeavyAttack,
            PlayerAnimState::Death,
        ] {
            assert_eq!(state.frame_seconds(), FRAME_SECONDS, "{state:?}");
        }
    }

    #[test]
    fn starting_and_resetting_a_clip_retimes_the_frame_clock() {
        let mut anim = PlayerAnimation::default();
        anim.start_action(PlayerAnimState::Water);
        assert_eq!(
            anim.frame_timer.duration().as_secs_f32(),
            PlayerAnimState::Water.frame_seconds()
        );
        anim.reset();
        assert_eq!(
            anim.frame_timer.duration().as_secs_f32(),
            PlayerAnimState::Idle.frame_seconds()
        );
    }

    #[test]
    fn a_looping_clip_wraps_back_to_the_first_frame() {
        let mut anim = PlayerAnimation::default();
        for _ in 0..FRAME_COUNT {
            step_animation(
                &mut anim,
                FRAME_SECONDS + 0.001,
                Vec2::ZERO,
                Vec2::ZERO,
                None,
            );
        }
        assert_eq!(anim.frame, 0);
    }

    #[test]
    fn a_one_shot_action_finishes_and_hands_back_to_locomotion() {
        let mut anim = PlayerAnimation::default();
        anim.start_action(PlayerAnimState::Water);
        assert!(anim.is_acting());

        for _ in 0..=FRAME_COUNT {
            step_animation(
                &mut anim,
                FRAME_SECONDS + 0.001,
                Vec2::ZERO,
                Vec2::ZERO,
                None,
            );
        }

        assert!(!anim.is_acting());
        assert_eq!(anim.state, PlayerAnimState::Idle);
    }

    #[test]
    fn death_holds_its_last_frame() {
        let mut anim = PlayerAnimation::default();
        anim.start_action(PlayerAnimState::Death);
        for _ in 0..(FRAME_COUNT * 2) {
            step_animation(&mut anim, FRAME_SECONDS + 0.001, Vec2::X, Vec2::X, None);
        }
        assert_eq!(anim.state, PlayerAnimState::Death);
        assert_eq!(anim.frame, FRAME_COUNT - 1);
    }

    #[test]
    fn reset_clears_an_action_back_to_idle() {
        let mut anim = PlayerAnimation::default();
        anim.start_action(PlayerAnimState::Death);
        anim.reset();
        assert_eq!(anim.state, PlayerAnimState::Idle);
        assert!(!anim.is_acting());
        assert_eq!(anim.frame, 0);
    }
}
