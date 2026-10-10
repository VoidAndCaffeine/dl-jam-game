//! The audio layer.
//!
//! Systems across the game request a cue by writing [`PlaySfx`] (a one-shot) or
//! [`PlayLoopSfx`] (a bounded loop). This plugin owns every decision about how a
//! cue actually sounds: which file(s), what volume, whether it is a sequence, a
//! random pick, a repeat, a fade, or a loop. Call sites only need the cue id.
//!
//! Behaviour that is bound to a live entity (the boss engine hum) or to an event
//! stream (menu open/close/navigate) is driven here too, so menus and gameplay
//! never have to know a single filename.
//!
//! Everything is skipped when there is no [`AssetServer`] (headless tests), so
//! spawning audio never needs a real backend.

use crate::components::boss::{Boss, BossId, DualRole, SurgeCharger};
use crate::events::{CropPlanted, PlayLoopSfx, PlaySfx, Sfx};
use crate::levels::LevelId;
use crate::resources::boss_select::BossSelectMenu;
use crate::resources::crafting_menu::CraftingMenu;
use crate::resources::crop_select::CropSelectMenu;
use crate::resources::inventory_panel::InventoryPanel;
use crate::resources::level::ActiveLevel;
use crate::resources::pause::PauseMenu;
use crate::states::{DayPhase, GameState, Phase};
use bevy::audio::{AudioSinkPlayback, Volume};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::time::Real;
use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};
use std::collections::{HashMap, VecDeque};

/// Volume tiers. "Quietly" cues (menu navigation, player death, the boss engine
/// hum, the caustic trail) use [`QUIET`]; most cues use [`NORMAL`]; the loudest
/// impacts use [`LOUD`].
pub const QUIET: f32 = 0.35;
pub const NORMAL: f32 = 0.7;
pub const LOUD: f32 = 1.0;

/// All sound files live here under the asset root.
const AUDIO_DIR: &str = "audio/";

/// Music volume, fade-in and fade-out times, in seconds.
const MUSIC_VOLUME: f32 = 0.6;
const MUSIC_FADE_IN: f32 = 1.6;
const MUSIC_FADE_OUT: f32 = 0.8;

// The music tracks, by context.
const MUSIC_MAIN_MENU: &str = "audio/music/06 - Continue.ogg";
const MUSIC_FARM: &str = "audio/music/mess.ogg";
const MUSIC_BOSS_A: &str = "audio/music/S31-High Alert.ogg";
const MUSIC_BOSS_B: &str = "audio/music/S31-Night Prowler.ogg";
const MUSIC_DUAL: &str = "audio/music/S31-Let the Games Begin.ogg";
const MUSIC_VICTORY: &str = "audio/music/Eye of the Storm.ogg";

/// How long `loop_water` plays at full volume before it fades.
const WATER_HOLD: f32 = 1.0;
/// How long the water loop takes to fade out.
const WATER_FADE_TOTAL: f32 = 0.2;
/// How long after the fade begins the splash starts (no gap).
const WATER_SPLASH_AT: f32 = 0.1;

/// The even/odd swosh pools. A light or heavy swing alternates parity, then
/// picks one of the three files of that parity at random.
const SWOSH_ODD: [&str; 3] = ["swosh-01.ogg", "swosh-03.ogg", "swosh-05.ogg"];
const SWOSH_EVEN: [&str; 3] = ["swosh-02.ogg", "swosh-04.ogg", "swosh-06.ogg"];

/// The metal/wood strikes a craft can be built from, with `(file, repeats, wood)`.
const CRAFT_STRIKES: [(&str, u32, bool); 6] = [
    ("wood6.ogg", 3, true),
    ("metal9.ogg", 2, false),
    ("metal5.ogg", 2, false),
    ("wood3.ogg", 1, true),
    ("metal2.ogg", 2, false),
    ("wood16.ogg", 2, true),
];

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySfx>()
            .add_message::<PlayLoopSfx>()
            .add_message::<CropPlanted>()
            .add_message::<crate::events::PlayerDied>()
            .init_resource::<AudioAssets>()
            .init_resource::<SwingAlternator>()
            .init_resource::<CraftRng>()
            .init_resource::<MenuWatch>()
            .init_resource::<MusicDirector>()
            .add_systems(
                Update,
                (
                    observe_menus,
                    drive_music,
                    resolve_player_death,
                    resolve_sfx_cues,
                    resolve_loop_sfx,
                    drive_water_fades,
                    drive_boss_engines,
                    drive_surge_sizzle,
                    advance_voices,
                    tick_timed_despawns,
                    tick_music_fade_in,
                    tick_music_fade_out,
                    sync_audio_pause,
                )
                    .chain(),
            )
            // A fight ending is the clearest moment loops must stop: whatever the
            // player was hearing, the farm has to come back silent.
            .add_systems(OnExit(DayPhase::BossFight), stop_loops_on_fight_exit)
            // Leaving play for the title screen or victory silences everything.
            .add_systems(OnExit(GameState::Playing), stop_audio_when_leaving_play);
    }
}

// --- Asset cache -------------------------------------------------------------

/// Lazily loaded sound handles, kept for the whole session (the set is small).
#[derive(Resource, Default)]
pub struct AudioAssets {
    cache: HashMap<&'static str, Handle<AudioSource>>,
}

impl AudioAssets {
    /// Loads (and caches) `file`, which is relative to `assets/audio`.
    fn handle(&mut self, server: &AssetServer, file: &'static str) -> Handle<AudioSource> {
        self.cache
            .entry(file)
            .or_insert_with(|| server.load(format!("{AUDIO_DIR}{file}")))
            .clone()
    }

    /// Loads (and caches) a path that already includes its folder (e.g. music).
    fn load_path(&mut self, server: &AssetServer, path: &'static str) -> Handle<AudioSource> {
        self.cache
            .entry(path)
            .or_insert_with(|| server.load(path))
            .clone()
    }
}

// --- Voice playback ----------------------------------------------------------

/// Tags a world/gameplay sound effect. These are silenced while a menu is open
/// and swept when play ends.
#[derive(Component)]
struct GameAudio;

/// Tags a menu sound effect (open/close/navigate/confirm, crafting, equipping).
/// These keep playing under menus, unlike [`GameAudio`].
#[derive(Component)]
struct MenuSfx;

/// Tags a looping audio entity so the fight-exit sweep can stop it early.
#[derive(Component)]
struct LoopVoice;

/// A one-shot voice, possibly part of a sequence. `remaining` holds the files
/// still to play after the one currently sounding.
#[derive(Component)]
struct Voice {
    remaining: VecDeque<Handle<AudioSource>>,
    volume: f32,
    /// Whether this is a menu sound (kept under menus) or a world sound.
    menu: bool,
}

/// Spawns a voice that plays the first handle and chains into `remaining`.
fn spawn_voice(
    commands: &mut Commands,
    mut handles: VecDeque<Handle<AudioSource>>,
    volume: f32,
    menu: bool,
) {
    let Some(first) = handles.pop_front() else {
        return;
    };
    let entity = commands
        .spawn((
            Name::new("Sfx Voice"),
            AudioPlayer::new(first),
            PlaybackSettings::ONCE.with_volume(Volume::Linear(volume)),
            Voice {
                remaining: handles,
                volume,
                menu,
            },
        ))
        .id();
    if menu {
        commands.entity(entity).insert(MenuSfx);
    } else {
        commands.entity(entity).insert(GameAudio);
    }
}

/// Plays a sequence of files back-to-back, each starting when the last ends.
fn spawn_sequence(
    commands: &mut Commands,
    assets: &mut AudioAssets,
    server: &AssetServer,
    files: &[&'static str],
    volume: f32,
    menu: bool,
) {
    let handles: VecDeque<Handle<AudioSource>> = files
        .iter()
        .map(|file| assets.handle(server, file))
        .collect();
    spawn_voice(commands, handles, volume, menu);
}

/// Advances a sequence: when the current clip finishes, the next entity starts.
fn advance_voices(
    mut commands: Commands,
    mut voices: Query<(Entity, &mut Voice, Option<&AudioSink>)>,
) {
    for (entity, mut voice, sink) in voices.iter_mut() {
        let Some(sink) = sink else {
            continue;
        };
        if !sink.empty() {
            continue;
        }
        let remaining = std::mem::take(&mut voice.remaining);
        let volume = voice.volume;
        let menu = voice.menu;
        commands.entity(entity).despawn();
        if !remaining.is_empty() {
            spawn_voice(&mut commands, remaining, volume, menu);
        }
    }
}

// --- Bounded loops -----------------------------------------------------------

/// A loop that stops and is removed after `0.0` seconds run out. These are a
/// safety net; the fight-exit sweep and pause handling are what stop loops
/// reliably, so a mistimed duration can never leave one running.
#[derive(Component)]
struct TimedDespawn(f32);

/// Plays the looping cues (wave bubbles, spray, amalgamation channel). Layered
/// cues spawn one entity per file. The caustic trail is driven by the boss's
/// charge instead (see [`drive_surge_sizzle`]).
fn resolve_loop_sfx(
    mut commands: Commands,
    server: Option<Res<AssetServer>>,
    mut assets: ResMut<AudioAssets>,
    mut cues: MessageReader<PlayLoopSfx>,
    menus: MenuOpenFlags,
) {
    let Some(server) = server else {
        for _ in cues.read() {}
        return;
    };
    // Loops are all world sounds, so they never begin under a menu.
    if menus.any_open() {
        for _ in cues.read() {}
        return;
    }
    for cue in cues.read() {
        let (files, volume) = loop_plan(cue.sfx);
        for file in files {
            let handle = assets.handle(&server, file);
            commands.spawn((
                Name::new("Loop Sfx"),
                GameAudio,
                LoopVoice,
                AudioPlayer::new(handle),
                PlaybackSettings::LOOP.with_volume(Volume::Linear(volume)),
                TimedDespawn(cue.duration),
            ));
        }
    }
}

fn tick_timed_despawns(
    time: Res<Time>,
    mut commands: Commands,
    mut loops: Query<(Entity, &mut TimedDespawn)>,
) {
    let dt = time.delta_secs();
    for (entity, mut life) in loops.iter_mut() {
        life.0 -= dt;
        if life.0 <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

// --- Explicit stop / pause ---------------------------------------------------

/// Stops every loop the moment a boss fight ends, so returning to the farm is
/// silent even if a pattern was mid-play (or a loop's timer was frozen by a
/// pause). One-shots such as the defeat sting are left alone.
fn stop_loops_on_fight_exit(mut commands: Commands, loops: Query<Entity, With<LoopVoice>>) {
    for entity in loops.iter() {
        commands.entity(entity).despawn();
    }
}

/// The menu resources, so systems can tell whether a menu owns the screen.
#[derive(SystemParam)]
struct MenuOpenFlags<'w> {
    pause: Option<Res<'w, PauseMenu>>,
    inventory: Option<Res<'w, InventoryPanel>>,
    crafting: Option<Res<'w, CraftingMenu>>,
    crop: Option<Res<'w, CropSelectMenu>>,
    boss: Option<Res<'w, BossSelectMenu>>,
}

impl MenuOpenFlags<'_> {
    fn any_open(&self) -> bool {
        self.pause.as_ref().is_some_and(|m| m.open)
            || self.inventory.as_ref().is_some_and(|m| m.open)
            || self.crafting.as_ref().is_some_and(|m| m.open)
            || self.crop.as_ref().is_some_and(|m| m.open)
            || self.boss.as_ref().is_some_and(|m| m.open)
    }
}

/// Silences every sound effect when play ends (title screen or victory), so no
/// cue can bleed out of a finished run. Music is deliberately left alone: it is
/// crossfaded to the new context by [`drive_music`].
/// Every sound-effect entity, gameplay or menu (music excluded).
type AllSfx<'w, 's> = Query<'w, 's, Entity, Or<(With<GameAudio>, With<MenuSfx>)>>;

fn stop_audio_when_leaving_play(mut commands: Commands, audio: AllSfx) {
    for entity in audio.iter() {
        commands.entity(entity).despawn();
    }
}

/// Holds world sound effects silent while a menu is open. Music and menu sounds
/// keep playing; this only touches [`GameAudio`]. Bevy's audio does not follow
/// [`Time<Virtual>`], so the sinking is done by hand.
fn sync_audio_pause(menus: MenuOpenFlags, mut sinks: Query<&mut AudioSink, With<GameAudio>>) {
    let paused = menus.any_open();
    for sink in sinks.iter_mut() {
        if paused && !sink.is_paused() {
            sink.pause();
        } else if !paused && sink.is_paused() {
            sink.play();
        }
    }
}

// --- The Excavator's caustic trail -------------------------------------------

/// Tracks the caustic-trail loop on a boss so it starts and stops with the
/// charge itself, rather than a guessed duration.
#[derive(Component, Default)]
struct SurgeSizzle {
    voice: Option<Entity>,
}

/// The bosses and their charge state, for [`drive_surge_sizzle`].
type SurgeBosses<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        Option<&'static SurgeCharger>,
        Option<&'static mut SurgeSizzle>,
    ),
    With<Boss>,
>;

/// Runs the quiet sizzle exactly while a boss carries a [`SurgeCharger`]. The
/// loop is a child of the boss, so it also dies with it.
fn drive_surge_sizzle(
    mut commands: Commands,
    server: Option<Res<AssetServer>>,
    mut assets: ResMut<AudioAssets>,
    mut bosses: SurgeBosses,
) {
    let Some(server) = server else {
        return;
    };
    for (entity, charger, sizzle) in bosses.iter_mut() {
        let Some(mut sizzle) = sizzle else {
            commands.entity(entity).insert(SurgeSizzle::default());
            continue;
        };
        let want = charger.is_some();
        if want == sizzle.voice.is_some() {
            continue;
        }
        if let Some(voice) = sizzle.voice.take() {
            commands.entity(voice).despawn();
        }
        if want {
            let handle = assets.handle(&server, "loop_rain.ogg");
            let voice = commands
                .spawn((
                    Name::new("Surge Sizzle"),
                    GameAudio,
                    LoopVoice,
                    AudioPlayer::new(handle),
                    PlaybackSettings::LOOP.with_volume(Volume::Linear(QUIET)),
                ))
                .id();
            commands.entity(entity).add_child(voice);
            sizzle.voice = Some(voice);
        }
    }
}

// --- The water two-part cue --------------------------------------------------

/// The watering sound: a loop of running water that fades into a splash.
#[derive(Component)]
struct WaterFade {
    /// Seconds elapsed in the current stage.
    elapsed: f32,
    /// Whether the hold is over and the fade has begun.
    fading: bool,
    /// Whether the splash has been spawned.
    splash: bool,
    /// The volume to fade down from.
    volume: f32,
}

fn drive_water_fades(
    time: Res<Time>,
    mut commands: Commands,
    server: Option<Res<AssetServer>>,
    mut assets: ResMut<AudioAssets>,
    mut voices: Query<(Entity, &mut WaterFade, &mut AudioSink)>,
) {
    let Some(server) = server else {
        return;
    };
    let dt = time.delta_secs();
    for (entity, mut water, mut sink) in voices.iter_mut() {
        water.elapsed += dt;
        if !water.fading {
            if water.elapsed >= WATER_HOLD {
                water.fading = true;
                water.elapsed = 0.0;
            }
            continue;
        }

        if !water.splash && water.elapsed >= WATER_SPLASH_AT {
            water.splash = true;
            let handle = assets.handle(&server, "splash_07.ogg");
            commands.spawn((
                Name::new("Water Splash"),
                GameAudio,
                AudioPlayer::new(handle),
                PlaybackSettings::DESPAWN.with_volume(Volume::Linear(NORMAL)),
            ));
        }

        let left = (1.0 - water.elapsed / WATER_FADE_TOTAL).clamp(0.0, 1.0);
        sink.set_volume(Volume::Linear(water.volume * left));
        if water.elapsed >= WATER_FADE_TOTAL {
            commands.entity(entity).despawn();
        }
    }
}

// --- Boss engine hum ---------------------------------------------------------

/// Tracks the ambient engine loop on a boss so it can follow the phase.
#[derive(Component, Default)]
struct BossEngine {
    /// The phase whose engine is currently applied (`None` = no engine).
    phase: Option<u8>,
    /// The spawned loop entity, if any.
    voice: Option<Entity>,
}

/// Keeps a phase-appropriate engine hum under the Excavator (single or the
/// dual fight's excavator half). The Mercurial has no engine art, so it stays
/// silent. The voice is parented to the boss, so it dies with it.
fn drive_boss_engines(
    mut commands: Commands,
    server: Option<Res<AssetServer>>,
    mut assets: ResMut<AudioAssets>,
    phase: Phase,
    mut bosses: Query<(Entity, &Boss, Option<&DualRole>, Option<&mut BossEngine>)>,
) {
    let Some(server) = server else {
        return;
    };
    if !phase.is_boss_fight() {
        return;
    }
    for (entity, boss, role, engine) in bosses.iter_mut() {
        let Some(mut engine) = engine else {
            commands.entity(entity).insert(BossEngine::default());
            continue;
        };
        let excavator = matches!(role, Some(DualRole::Excavator))
            || (role.is_none() && boss.id == BossId::BossA);
        let want = if excavator {
            Some(boss.phase.clamp(1, 2))
        } else {
            None
        };
        if engine.phase == want {
            continue;
        }
        if let Some(voice) = engine.voice.take() {
            commands.entity(voice).despawn();
        }
        engine.voice = want.map(|phase| {
            let file = if phase >= 2 {
                "spaceEngineSmall_000.ogg"
            } else {
                "spaceEngineSmall_002.ogg"
            };
            let handle = assets.handle(&server, file);
            let voice = commands
                .spawn((
                    Name::new("Boss Engine"),
                    GameAudio,
                    LoopVoice,
                    AudioPlayer::new(handle),
                    PlaybackSettings::LOOP.with_volume(Volume::Linear(QUIET)),
                ))
                .id();
            commands.entity(entity).add_child(voice);
            voice
        });
        engine.phase = want;
    }
}

// --- Music -------------------------------------------------------------------

/// A looping music track that fades up to `target` over `fade_in` seconds.
#[derive(Component)]
struct MusicVoice {
    target: f32,
    level: f32,
    fade_in: f32,
}

/// A music track that is fading out and should be removed when silent.
#[derive(Component)]
struct MusicFadeOut {
    remaining: f32,
    from: f32,
}

/// The track that should be playing for the current context.
#[derive(Resource, Default)]
struct MusicDirector {
    current: Option<&'static str>,
    voice: Option<Entity>,
}

/// The music context maps to exactly one track: the title screen, the victory
/// screen, and (in play) the farm or whichever boss is being fought.
fn desired_track(
    state: Option<&State<GameState>>,
    day_phase: Option<&State<DayPhase>>,
    active: Option<&ActiveLevel>,
) -> Option<&'static str> {
    match state?.get() {
        GameState::MainMenu => Some(MUSIC_MAIN_MENU),
        GameState::Victory => Some(MUSIC_VICTORY),
        GameState::Playing => match day_phase?.get() {
            DayPhase::BossFight => Some(match active?.id {
                LevelId::ArenaA => MUSIC_BOSS_A,
                LevelId::ArenaB => MUSIC_BOSS_B,
                LevelId::ArenaDual => MUSIC_DUAL,
                LevelId::Farm => MUSIC_FARM,
            }),
            _ => Some(MUSIC_FARM),
        },
        GameState::LoadingAssets => None,
    }
}

/// Crossfades to the track the current context wants. A context change fades the
/// old track out and the new one in; opening a menu does not change the context,
/// so music simply keeps playing under menus.
fn drive_music(
    mut commands: Commands,
    server: Option<Res<AssetServer>>,
    mut assets: ResMut<AudioAssets>,
    state: Option<Res<State<GameState>>>,
    day_phase: Option<Res<State<DayPhase>>>,
    active: Option<Res<ActiveLevel>>,
    mut director: ResMut<MusicDirector>,
) {
    let Some(server) = server else {
        return;
    };
    let desired = desired_track(state.as_deref(), day_phase.as_deref(), active.as_deref());
    if director.current == desired {
        return;
    }

    if let Some(voice) = director.voice.take() {
        commands.entity(voice).insert(MusicFadeOut {
            remaining: MUSIC_FADE_OUT,
            from: MUSIC_VOLUME,
        });
    }
    director.current = desired;
    director.voice = desired.map(|path| {
        let handle = assets.load_path(&server, path);
        commands
            .spawn((
                Name::new("Music"),
                MusicVoice {
                    target: MUSIC_VOLUME,
                    level: 0.0,
                    fade_in: MUSIC_FADE_IN,
                },
                AudioPlayer::new(handle),
                PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
            ))
            .id()
    });
}

/// Fades music tracks up to their target volume. Runs on real time so the fade
/// is unaffected by a paused game.
fn tick_music_fade_in(time: Res<Time<Real>>, mut voices: Query<(&mut MusicVoice, &mut AudioSink)>) {
    let dt = time.delta_secs();
    for (mut music, mut sink) in voices.iter_mut() {
        if music.level >= music.target {
            continue;
        }
        music.level =
            (music.level + music.target * dt / music.fade_in.max(0.001)).min(music.target);
        sink.set_volume(Volume::Linear(music.level));
    }
}

/// Fades outgoing music down and removes it when silent.
fn tick_music_fade_out(
    time: Res<Time<Real>>,
    mut commands: Commands,
    mut voices: Query<(Entity, &mut MusicFadeOut, &mut AudioSink)>,
) {
    let dt = time.delta_secs();
    for (entity, mut fade, mut sink) in voices.iter_mut() {
        fade.remaining -= dt;
        let left = (fade.remaining / MUSIC_FADE_OUT).clamp(0.0, 1.0);
        sink.set_volume(Volume::Linear(fade.from * left));
        if fade.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

// --- Cue resolution ----------------------------------------------------------

/// The player's defeat rattle. Kept central so every source of [`PlayerDied`]
/// (boss attacks, splash, friendly fire) sounds the same.
fn resolve_player_death(
    mut died: MessageReader<crate::events::PlayerDied>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    for _ in died.read() {
        sfx.write(PlaySfx(Sfx::PlayerDeath));
    }
}

/// One-shot / sequence / random resolution. Stateful cues (swings, crafts) are
/// handled here so the caller never has to.
fn resolve_sfx_cues(
    mut commands: Commands,
    server: Option<Res<AssetServer>>,
    mut assets: ResMut<AudioAssets>,
    mut cues: MessageReader<PlaySfx>,
    mut swing: ResMut<SwingAlternator>,
    mut craft: ResMut<CraftRng>,
    menus: MenuOpenFlags,
) {
    let Some(server) = server else {
        for _ in cues.read() {}
        return;
    };
    for cue in cues.read() {
        // A world sound never starts while a menu owns the screen; a menu sound
        // always may. (Menu === MenuOpenFlags; music is handled separately.)
        if !is_menu_sfx(cue.0) && menus.any_open() {
            continue;
        }
        match cue.0 {
            Sfx::PlayerSwingLight => {
                let file = swing.next(true);
                spawn_sequence(&mut commands, &mut assets, &server, &[file], NORMAL, false);
            }
            Sfx::PlayerSwingHeavy => {
                let file = swing.next(false);
                spawn_sequence(&mut commands, &mut assets, &server, &[file], NORMAL, false);
            }
            Sfx::GearCraft => {
                let files = craft.craft_sequence();
                spawn_sequence(&mut commands, &mut assets, &server, &files, NORMAL, true);
            }
            Sfx::CropWaterStart => {
                let handle = assets.handle(&server, "loop_water_03.ogg");
                commands.spawn((
                    Name::new("Water Loop"),
                    GameAudio,
                    LoopVoice,
                    AudioPlayer::new(handle),
                    PlaybackSettings::LOOP.with_volume(Volume::Linear(NORMAL)),
                    WaterFade {
                        elapsed: 0.0,
                        fading: false,
                        splash: false,
                        volume: NORMAL,
                    },
                ));
            }
            other => {
                let (files, volume) = one_shot_plan(other);
                spawn_sequence(
                    &mut commands,
                    &mut assets,
                    &server,
                    &files,
                    volume,
                    is_menu_sfx(other),
                );
            }
        }
    }
}

/// Menu-context cues: UI blips plus the forge/inventory actions, all of which
/// happen with a menu open and so must not be silenced by one.
fn is_menu_sfx(sfx: Sfx) -> bool {
    matches!(
        sfx,
        Sfx::MenuNavigate
            | Sfx::MenuOpen
            | Sfx::MenuConfirm
            | Sfx::MenuClose
            | Sfx::GearCraft
            | Sfx::GearEquip
    )
}

/// The files and volume for a plain one-shot cue (sequences included).
fn one_shot_plan(sfx: Sfx) -> (Vec<&'static str>, f32) {
    use Sfx::*;
    match sfx {
        // Boss A
        SurgeWindup => (vec!["diesel-horn.ogg"], NORMAL),
        SurgeCharge => (vec!["spaceEngine_001.ogg"], NORMAL),
        SlamWindup => (vec!["doorClose_2.ogg"], NORMAL),
        SlamImpact => (vec!["explosionCrunch_001.ogg"], LOUD),
        AcidForm => (vec!["AcidForm.ogg"], NORMAL),
        DebrisWarning => (vec!["lowFrequency_explosion_000.ogg"], NORMAL),
        DebrisImpact => (vec!["explosionCrunch_000.ogg"], NORMAL),
        // Boss B
        Blink => (vec!["teleport.ogg"], NORMAL),
        DecoySpawn => (vec!["confirmation_004.ogg"], NORMAL),
        DecoyPop => (vec!["bubble_02.ogg"], NORMAL),
        SprayWindup => (vec!["forceField_000.ogg"], NORMAL),
        MadnessApply => (vec!["confirmation_002.ogg"], NORMAL),
        // Dual
        AmalgamWarning => (vec!["forceField_001.ogg"; 3], NORMAL),
        AmalgamExplode => (vec!["explosionCrunch_003.ogg"], LOUD),
        // Shared combat
        BossHit => (vec!["dropLeather.ogg"], NORMAL),
        PlayerHit => (vec!["footstep09.ogg"], NORMAL),
        BossDefeated => (vec!["load_into_game.ogg"], NORMAL),
        PlayerDeath => (vec!["death19.ogg"], QUIET),
        BossADeath => (vec!["steam_hisses.ogg"], NORMAL),
        BossBDeath => (vec!["slime_10.ogg"], NORMAL),
        // Farming
        CropPlant => (vec!["cloth2.ogg", "cloth3.ogg", "cloth4.ogg"], NORMAL),
        CropHarvest => (
            vec!["drawKnife2.ogg", "metalClick.ogg", "handleSmallLeather.ogg"],
            NORMAL,
        ),
        // Crafting
        GearEquip => (vec!["clothBelt.ogg"], NORMAL),
        // Menu / UI
        MenuNavigate => (vec!["BookFlip5.ogg"], QUIET),
        MenuOpen => (vec!["BookFlip3.ogg"], NORMAL),
        MenuConfirm => (vec!["load_into_game.ogg"], NORMAL),
        MenuClose => (vec!["BookFlip6.ogg"], NORMAL),
        // World
        DayAdvance => (vec!["doorOpen_2.ogg", "doorClose_4.ogg"], NORMAL),
        // Loops and stateful cues never reach here; fall back to silence.
        TailingsSizzle | ExcavatorEngine1 | ExcavatorEngine2 | WaveLaunch | SprayRelease
        | AmalgamChannel | PlayerSwingLight | PlayerSwingHeavy | GearCraft | CropWaterStart
        | CropWaterEnd => (Vec::new(), NORMAL),
    }
}

/// The files and volume for a looping cue.
fn loop_plan(sfx: Sfx) -> (Vec<&'static str>, f32) {
    use Sfx::*;
    match sfx {
        TailingsSizzle => (vec!["loop_rain.ogg"], QUIET),
        WaveLaunch => (vec!["loop_bubbles_1.ogg"], NORMAL),
        SprayRelease => (vec!["computerNoise_003.ogg"], NORMAL),
        AmalgamChannel => (
            vec!["engineCircular_000.ogg", "thrusterFire_003.ogg"],
            NORMAL,
        ),
        _ => (Vec::new(), NORMAL),
    }
}

// --- Swing alternation and craft randomisation -------------------------------

/// Alternates the parity of swosh cues: after an odd one comes an even one, and
/// vice versa. A random file within the required parity is chosen each time.
#[derive(Resource)]
pub struct SwingAlternator {
    rng: SmallRng,
    /// Whether the next swing should use the odd pool.
    next_odd: bool,
}

impl Default for SwingAlternator {
    fn default() -> Self {
        Self {
            rng: SmallRng::seed_from_u64(0x5005_0000_0000_0001),
            next_odd: true,
        }
    }
}

impl SwingAlternator {
    /// The next swosh file for a `light` swing (light uses odd parity first).
    fn next(&mut self, _light: bool) -> &'static str {
        let pool = if self.next_odd { SWOSH_ODD } else { SWOSH_EVEN };
        self.next_odd = !self.next_odd;
        pool[self.rng.random_range(0..pool.len())]
    }
}

/// Builds a randomised crafting strike sequence.
#[derive(Resource)]
pub struct CraftRng(SmallRng);

impl Default for CraftRng {
    fn default() -> Self {
        Self(SmallRng::seed_from_u64(0xC0FF_EE00_1234_5678))
    }
}

impl CraftRng {
    /// Picks three distinct strikes (at least one wood and one metal) in random
    /// order, then expands each into its repeat count, back-to-back.
    fn craft_sequence(&mut self) -> Vec<&'static str> {
        let wood: Vec<usize> = (0..CRAFT_STRIKES.len())
            .filter(|&i| CRAFT_STRIKES[i].2)
            .collect();
        let metal: Vec<usize> = (0..CRAFT_STRIKES.len())
            .filter(|&i| !CRAFT_STRIKES[i].2)
            .collect();

        // One wood + two metal, or two wood + one metal, chosen at random.
        let (wood_count, metal_count) = if self.0.random_range(0..2) == 0 {
            (1, 2)
        } else {
            (2, 1)
        };

        let mut chosen: Vec<usize> = Vec::with_capacity(3);
        self.sample(&wood, wood_count, &mut chosen);
        self.sample(&metal, metal_count, &mut chosen);

        // Shuffle the chosen strikes so the wood/metal order varies.
        for i in (1..chosen.len()).rev() {
            let j = self.0.random_range(0..=i);
            chosen.swap(i, j);
        }

        let mut files = Vec::new();
        for index in chosen {
            let (file, repeats, _) = CRAFT_STRIKES[index];
            for _ in 0..repeats {
                files.push(file);
            }
        }
        files
    }

    /// Samples `count` distinct entries from `pool` into `out`.
    fn sample(&mut self, pool: &[usize], count: usize, out: &mut Vec<usize>) {
        let mut pool = pool.to_vec();
        for _ in 0..count {
            if pool.is_empty() {
                break;
            }
            let index = self.0.random_range(0..pool.len());
            out.push(pool.swap_remove(index));
        }
    }
}

// --- Menu observation --------------------------------------------------------

/// One panel's open/selection state, tracked frame to frame.
#[derive(Default, Clone, Copy)]
struct PanelState {
    open: bool,
    selected: usize,
}

/// Watches every menu resource and turns open/close/selection changes into
/// [`Sfx::MenuOpen`], [`Sfx::MenuClose`] and [`Sfx::MenuNavigate`]. Centralised
/// here so no menu system has to emit audio itself.
#[derive(Resource, Default)]
struct MenuWatch {
    crafting: PanelState,
    inventory: PanelState,
    crop: PanelState,
    boss: PanelState,
    pause: PanelState,
    victory_selected: usize,
    primed: bool,
}

#[allow(clippy::too_many_arguments)]
fn observe_menus(
    mut sfx: MessageWriter<PlaySfx>,
    crafting: Option<Res<CraftingMenu>>,
    inventory: Option<Res<InventoryPanel>>,
    crop: Option<Res<CropSelectMenu>>,
    boss: Option<Res<BossSelectMenu>>,
    pause: Option<Res<PauseMenu>>,
    game: Option<Res<State<GameState>>>,
    victory: Option<Res<crate::plugins::day_cycle::VictorySelection>>,
    mut planted: MessageReader<CropPlanted>,
    mut watch: ResMut<MenuWatch>,
) {
    // A plant closes the picker the same moment, so its close must be silent.
    let mut planted_now = false;
    for _ in planted.read() {
        planted_now = true;
    }

    let crafting = crafting.map(|m| PanelState {
        open: m.open,
        selected: m.selected,
    });
    let inventory = inventory.map(|p| PanelState {
        open: p.open,
        selected: p.selected,
    });
    let crop = crop.map(|m| PanelState {
        open: m.open,
        selected: m.selected,
    });
    let boss = boss.map(|m| PanelState {
        open: m.open,
        selected: m.selected,
    });
    let pause = pause.map(|p: Res<PauseMenu>| PanelState {
        open: p.open,
        selected: p.selected,
    });

    // First frame just records the state; nothing should sound on boot.
    if !watch.primed {
        if let Some(state) = crafting {
            watch.crafting = state;
        }
        if let Some(state) = inventory {
            watch.inventory = state;
        }
        if let Some(state) = crop {
            watch.crop = state;
        }
        if let Some(state) = boss {
            watch.boss = state;
        }
        if let Some(state) = pause {
            watch.pause = state;
        }
        watch.victory_selected = victory.as_ref().map(|v| v.selected).unwrap_or(0);
        watch.primed = true;
        return;
    }

    if let Some(state) = crafting {
        observe_panel(state, &mut watch.crafting, &mut sfx);
    }
    if let Some(state) = inventory {
        observe_panel(state, &mut watch.inventory, &mut sfx);
    }
    if let Some(state) = crop {
        // Planting closes the picker and plays the plant sound; adding the menu
        // close chirp on top would overplay it, so that one close stays silent.
        if crop_close_is_silent(watch.crop.open, state.open, planted_now) {
            watch.crop = state;
        } else {
            observe_panel(state, &mut watch.crop, &mut sfx);
        }
    }
    if let Some(state) = boss {
        observe_panel(state, &mut watch.boss, &mut sfx);
    }
    if let Some(state) = pause {
        observe_panel(state, &mut watch.pause, &mut sfx);
    }

    if let Some(victory) = victory {
        let on_victory = game
            .as_ref()
            .is_some_and(|g| *g.get() == GameState::Victory);
        if on_victory && victory.selected != watch.victory_selected {
            sfx.write(PlaySfx(Sfx::MenuNavigate));
        }
        watch.victory_selected = victory.selected;
    }
}

/// Whether a crop-picker close should be silent: it closed, and a crop was just
/// planted (so the plant sound already covers it).
fn crop_close_is_silent(was_open: bool, now_open: bool, planted: bool) -> bool {
    was_open && !now_open && planted
}

/// Emits the right cue for a single panel's transition.
fn observe_panel(state: PanelState, prev: &mut PanelState, sfx: &mut MessageWriter<PlaySfx>) {
    if state.open && !prev.open {
        sfx.write(PlaySfx(Sfx::MenuOpen));
    } else if !state.open && prev.open {
        sfx.write(PlaySfx(Sfx::MenuClose));
    } else if state.open && prev.open && state.selected != prev.selected {
        sfx.write(PlaySfx(Sfx::MenuNavigate));
    }
    *prev = state;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_odd(file: &str) -> bool {
        SWOSH_ODD.contains(&file)
    }

    #[test]
    fn swings_alternate_between_odd_and_even_pools() {
        let mut swing = SwingAlternator::default();
        let mut previous: Option<bool> = None;
        for _ in 0..8 {
            let file = swing.next(true);
            let odd = is_odd(file);
            assert!(
                SWOSH_ODD.contains(&file) || SWOSH_EVEN.contains(&file),
                "unknown swosh {file}"
            );
            if let Some(previous) = previous {
                assert_ne!(odd, previous, "parity must alternate every swing");
            }
            previous = Some(odd);
        }
    }

    #[test]
    fn a_craft_picks_three_distinct_strikes_with_wood_and_metal() {
        let mut craft = CraftRng::default();
        for _ in 0..32 {
            let sequence = craft.craft_sequence();
            assert!(!sequence.is_empty());

            // Count how many times each strike appears; a present file must show
            // exactly its authored repeat count.
            let mut present: Vec<&str> = Vec::new();
            for (file, repeats, _) in CRAFT_STRIKES {
                let count = sequence.iter().filter(|f| **f == file).count();
                assert!(
                    count == 0 || count == repeats as usize,
                    "{file} appeared {count} times, expected 0 or {repeats}"
                );
                if count > 0 {
                    present.push(file);
                }
            }
            assert_eq!(present.len(), 3, "three distinct strikes: {present:?}");

            let wood = present
                .iter()
                .filter(|file| CRAFT_STRIKES.iter().any(|(f, _, w)| f == *file && *w))
                .count();
            let metal = present.len() - wood;
            assert!(wood >= 1 && metal >= 1, "needs wood and metal: {present:?}");

            let expected_len: usize = present
                .iter()
                .map(|file| {
                    CRAFT_STRIKES
                        .iter()
                        .find(|(f, _, _)| f == file)
                        .map(|(_, repeats, _)| *repeats as usize)
                        .unwrap_or(0)
                })
                .sum();
            assert_eq!(sequence.len(), expected_len);
        }
    }

    #[test]
    fn key_one_shot_plans_match_their_files() {
        assert_eq!(
            one_shot_plan(Sfx::CropPlant).0,
            vec!["cloth2.ogg", "cloth3.ogg", "cloth4.ogg"]
        );
        assert_eq!(
            one_shot_plan(Sfx::CropHarvest).0,
            vec!["drawKnife2.ogg", "metalClick.ogg", "handleSmallLeather.ogg"]
        );
        assert_eq!(
            one_shot_plan(Sfx::AmalgamWarning).0,
            vec!["forceField_001.ogg"; 3]
        );
        assert_eq!(
            one_shot_plan(Sfx::DayAdvance).0,
            vec!["doorOpen_2.ogg", "doorClose_4.ogg"]
        );
    }

    #[test]
    fn key_loop_plans_match_their_files() {
        assert_eq!(loop_plan(Sfx::TailingsSizzle).0, vec!["loop_rain.ogg"]);
        assert_eq!(loop_plan(Sfx::WaveLaunch).0, vec!["loop_bubbles_1.ogg"]);
        assert_eq!(
            loop_plan(Sfx::SprayRelease).0,
            vec!["computerNoise_003.ogg"]
        );
        assert_eq!(
            loop_plan(Sfx::AmalgamChannel).0,
            vec!["engineCircular_000.ogg", "thrusterFire_003.ogg"]
        );
    }

    #[test]
    fn leaving_a_fight_stops_loops_but_spares_one_shots() {
        let mut app = App::new();
        app.add_systems(Update, stop_loops_on_fight_exit);

        let loop_voice = app.world_mut().spawn((GameAudio, LoopVoice)).id();
        let one_shot = app.world_mut().spawn(GameAudio).id();
        app.update();

        let world = app.world();
        assert!(world.get::<LoopVoice>(loop_voice).is_none());
        assert!(
            world.get::<GameAudio>(one_shot).is_some(),
            "a defeat sting must be allowed to finish"
        );
    }

    #[test]
    fn leaving_play_stops_everything() {
        let mut app = App::new();
        app.add_systems(Update, stop_audio_when_leaving_play);

        let looping = app.world_mut().spawn((GameAudio, LoopVoice)).id();
        let one_shot = app.world_mut().spawn(GameAudio).id();
        app.update();

        let world = app.world();
        assert!(world.get::<GameAudio>(looping).is_none());
        assert!(world.get::<GameAudio>(one_shot).is_none());
    }

    #[test]
    fn planting_silences_the_picker_close() {
        assert!(
            crop_close_is_silent(true, false, true),
            "plant close is silent"
        );
        assert!(
            !crop_close_is_silent(true, false, false),
            "a cancel still plays the close chirp"
        );
        assert!(
            !crop_close_is_silent(true, true, true),
            "no close means nothing to silence"
        );
        assert!(
            !crop_close_is_silent(false, true, true),
            "opening is not a close"
        );
    }

    #[test]
    fn menu_cues_keep_playing_under_menus_but_world_cues_do_not() {
        for sfx in [
            Sfx::MenuNavigate,
            Sfx::MenuOpen,
            Sfx::MenuConfirm,
            Sfx::MenuClose,
            Sfx::GearCraft,
            Sfx::GearEquip,
        ] {
            assert!(is_menu_sfx(sfx), "{sfx:?} should count as a menu sound");
        }
        for sfx in [
            Sfx::BossHit,
            Sfx::SlamImpact,
            Sfx::CropHarvest,
            Sfx::PlayerSwingLight,
            Sfx::Blink,
        ] {
            assert!(!is_menu_sfx(sfx), "{sfx:?} should count as a world sound");
        }
    }

    #[test]
    fn every_music_track_is_an_ogg_in_the_music_folder() {
        for track in [
            MUSIC_MAIN_MENU,
            MUSIC_FARM,
            MUSIC_BOSS_A,
            MUSIC_BOSS_B,
            MUSIC_DUAL,
            MUSIC_VICTORY,
        ] {
            assert!(track.starts_with("audio/music/"), "{track}");
            assert!(track.ends_with(".ogg"), "{track}");
        }
    }

    #[test]
    fn the_music_track_follows_the_game_context() {
        use bevy::state::app::StatesPlugin;

        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .init_resource::<crate::resources::level::ActiveLevel>();

        let track = |app: &mut App| {
            let state = app.world().resource::<State<GameState>>();
            let phase = app.world().resource::<State<DayPhase>>();
            let active = app
                .world()
                .resource::<crate::resources::level::ActiveLevel>();
            desired_track(Some(state), Some(phase), Some(active))
        };

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::MainMenu);
        app.update();
        assert_eq!(track(&mut app), Some(MUSIC_MAIN_MENU));

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();
        assert_eq!(track(&mut app), Some(MUSIC_VICTORY));

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);
        app.update();
        assert_eq!(track(&mut app), Some(MUSIC_FARM));

        for (arena, expected) in [
            (LevelId::ArenaA, MUSIC_BOSS_A),
            (LevelId::ArenaB, MUSIC_BOSS_B),
            (LevelId::ArenaDual, MUSIC_DUAL),
        ] {
            app.world_mut()
                .resource_mut::<crate::resources::level::ActiveLevel>()
                .id = arena;
            app.world_mut()
                .resource_mut::<NextState<DayPhase>>()
                .set(DayPhase::BossFight);
            app.update();
            assert_eq!(track(&mut app), Some(expected), "{arena:?}");
            app.world_mut()
                .resource_mut::<NextState<DayPhase>>()
                .set(DayPhase::Farming);
            app.update();
        }
    }

    #[test]
    fn quiet_cues_are_quieter_than_normal_ones() {
        assert_eq!(one_shot_plan(Sfx::MenuNavigate).1, QUIET);
        assert_eq!(one_shot_plan(Sfx::PlayerDeath).1, QUIET);
        assert_eq!(loop_plan(Sfx::TailingsSizzle).1, QUIET);
        assert_eq!(one_shot_plan(Sfx::BossHit).1, NORMAL);
        assert_eq!(one_shot_plan(Sfx::SlamImpact).1, LOUD);
    }
}
