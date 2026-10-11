//! The black fade that covers a scene change.
//!
//! A scene change reads as a hard cut otherwise: the old scene is up one frame
//! and the new one the next. This plugin fades the screen to black, performs the
//! change while it is dark, and fades the new scene back in. The fade runs on
//! real time so a paused overlay cannot freeze it.
//!
//! Two kinds of change are covered:
//!   * a farm ↔ arena swap, which moves the game into the loading phase while
//!     dark (the caller fills [`LoadingContext`] first), and
//!   * a top-level game-state change (starting a run, leaving the victory screen).
//!
//! Callers request a fade through [`SceneFade::begin_load`] or
//! [`SceneFade::begin_game`] instead of changing state directly, which is what
//! buys the fade-out its time on screen. Headless runs (no renderer) swap at
//! once.

use crate::resources::scene_assets::{LoadTarget, LoadingContext};
use crate::states::{DayPhase, GameState, Phase};
use bevy::asset::AssetServer;
use bevy::prelude::*;
use bevy::time::Real;
use bevy::ui::FocusPolicy;
use std::time::Duration;

/// Seconds to fade the old scene out, and the new scene back in.
const FADE_OUT: f32 = 0.28;
const FADE_IN: f32 = 0.32;
/// Above the loading screen (1000) and every menu, so nothing shows through.
const FADE_Z: i32 = 2000;

/// The fade's state machine.
#[derive(Default, PartialEq, Debug)]
enum FadeState {
    #[default]
    Idle,
    /// Darkening toward black over the timer.
    FadingOut(Timer),
    /// Fully black; waiting for the change to land.
    Black,
    /// Brightening back over the timer.
    FadingIn(Timer),
}

/// What the fade does at the dark point.
#[derive(Clone, PartialEq, Debug)]
enum FadeSwitch {
    /// Enter the loading phase; [`LoadingContext`] already holds the scene.
    Load,
    /// Change the top-level game state.
    Game(GameState),
}

/// Drives the fade and performs the pending change at the dark point.
#[derive(Resource, Default)]
pub struct SceneFade {
    state: FadeState,
    switch: Option<FadeSwitch>,
}

impl SceneFade {
    /// True when no fade is running, so a new one may start.
    pub fn is_idle(&self) -> bool {
        self.state == FadeState::Idle
    }

    /// Fades to black, then loads `target` and resumes `resume`. Ignored while a
    /// fade is already running, so a stray second request cannot cut the first
    /// one short.
    pub fn begin_load(
        &mut self,
        target: LoadTarget,
        resume: DayPhase,
        context: &mut LoadingContext,
    ) {
        if !self.is_idle() {
            return;
        }
        context.target = Some(target);
        context.resume_phase = Some(resume);
        self.switch = Some(FadeSwitch::Load);
        self.state = FadeState::FadingOut(Timer::from_seconds(FADE_OUT, TimerMode::Once));
    }

    /// Fades to black, then swaps the top-level game state. Ignored while a fade
    /// is already running.
    pub fn begin_game(&mut self, game: GameState) {
        if !self.is_idle() {
            return;
        }
        self.switch = Some(FadeSwitch::Game(game));
        self.state = FadeState::FadingOut(Timer::from_seconds(FADE_OUT, TimerMode::Once));
    }
}

/// The full-screen black rectangle.
#[derive(Component)]
struct FadeOverlay;

pub struct SceneFadePlugin;

impl Plugin for SceneFadePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneFade>()
            // Runs after every `Update` system, so a fade requested this frame is
            // driven the same frame instead of racing the system that asked.
            .add_systems(PostUpdate, drive_scene_fade);
    }
}

/// Performs the pending change at the dark point.
fn apply_switch(
    switch: &Option<FadeSwitch>,
    next_phase: Option<&mut NextState<DayPhase>>,
    next_game: &mut NextState<GameState>,
) {
    match switch {
        Some(FadeSwitch::Load) => {
            // `DayPhase` only exists inside `Playing`, which every load is.
            if let Some(next) = next_phase {
                next.set(DayPhase::Loading);
            }
        }
        Some(FadeSwitch::Game(game)) => next_game.set(game.clone()),
        None => {}
    }
}

/// Whether the pending change has landed, so the scene can be revealed.
fn arrived(switch: &Option<FadeSwitch>, phase: &Phase) -> bool {
    match switch {
        Some(FadeSwitch::Load) => phase.day_phase() != Some(DayPhase::Loading),
        Some(FadeSwitch::Game(game)) => phase.game.get() == game,
        None => true,
    }
}

/// Advances the fade and performs the change while the screen is dark.
#[allow(clippy::too_many_arguments)]
fn drive_scene_fade(
    mut commands: Commands,
    time: Res<Time<Real>>,
    server: Option<Res<AssetServer>>,
    mut fade: ResMut<SceneFade>,
    mut next_phase: Option<ResMut<NextState<DayPhase>>>,
    mut next_game: ResMut<NextState<GameState>>,
    phase: Phase,
    mut overlays: Query<(Entity, &mut BackgroundColor), With<FadeOverlay>>,
) {
    // No renderer (headless tests): there is nothing to fade, so swap at once.
    if server.is_none() {
        let switch = fade.switch.clone();
        if matches!(fade.state, FadeState::FadingOut(_)) {
            apply_switch(&switch, next_phase.as_deref_mut(), &mut next_game);
            fade.state = FadeState::Black;
        } else if fade.state == FadeState::Black && arrived(&switch, &phase) {
            fade.state = FadeState::Idle;
            fade.switch = None;
        }
        return;
    }

    let dt = time.delta_secs();
    let switch = fade.switch.clone();
    let mut state = std::mem::take(&mut fade.state);
    match &mut state {
        FadeState::Idle => {
            for (entity, _) in overlays.iter() {
                commands.entity(entity).despawn();
            }
        }
        FadeState::FadingOut(timer) => {
            timer.tick(Duration::from_secs_f32(dt));
            set_overlay(&mut commands, &mut overlays, timer.fraction());
            if timer.is_finished() {
                apply_switch(&switch, next_phase.as_deref_mut(), &mut next_game);
                state = FadeState::Black;
            }
        }
        FadeState::Black => {
            set_overlay(&mut commands, &mut overlays, 1.0);
            // The change has landed: reveal the new scene.
            if arrived(&switch, &phase) {
                state = FadeState::FadingIn(Timer::from_seconds(FADE_IN, TimerMode::Once));
            }
        }
        FadeState::FadingIn(timer) => {
            timer.tick(Duration::from_secs_f32(dt));
            set_overlay(&mut commands, &mut overlays, 1.0 - timer.fraction());
            if timer.is_finished() {
                for (entity, _) in overlays.iter() {
                    commands.entity(entity).despawn();
                }
                state = FadeState::Idle;
            }
        }
    }
    fade.state = state;
    if fade.state == FadeState::Idle {
        fade.switch = None;
    }
}

/// Sets the overlay's alpha, spawning it the first time it is needed.
fn set_overlay(
    commands: &mut Commands,
    overlays: &mut Query<(Entity, &mut BackgroundColor), With<FadeOverlay>>,
    alpha: f32,
) {
    let alpha = alpha.clamp(0.0, 1.0);
    if let Some((_, mut color)) = overlays.iter_mut().next() {
        color.0 = Color::srgba(0.0, 0.0, 0.0, alpha);
        return;
    }
    commands.spawn((
        Name::new("Scene Fade"),
        FadeOverlay,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, alpha)),
        FocusPolicy::Block,
        GlobalZIndex(FADE_Z),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::loading::LoadingPlugin;
    use crate::resources::run_data::PlayerGear;
    use bevy::state::app::StatesPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin, SceneFadePlugin, LoadingPlugin))
            .init_resource::<PlayerGear>()
            .init_state::<GameState>()
            .init_state::<DayPhase>();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        app
    }

    fn begin_load(app: &mut App, target: LoadTarget, resume: DayPhase) {
        app.world_mut()
            .resource_scope::<SceneFade, _>(|world, mut fade| {
                let mut context = world.resource_mut::<LoadingContext>();
                fade.begin_load(target, resume, &mut context);
            });
    }

    fn phase(app: &App) -> DayPhase {
        app.world().resource::<State<DayPhase>>().get().clone()
    }

    fn game(app: &App) -> GameState {
        app.world().resource::<State<GameState>>().get().clone()
    }

    #[test]
    fn headless_load_transitions_are_immediate() {
        let mut app = setup_app();
        begin_load(&mut app, LoadTarget::ArenaA, DayPhase::BossFight);

        for _ in 0..3 {
            app.update();
        }

        assert_eq!(phase(&app), DayPhase::BossFight);
        assert!(app.world().resource::<SceneFade>().is_idle());
    }

    #[test]
    fn begin_load_records_the_target_and_resume() {
        let mut app = setup_app();
        begin_load(&mut app, LoadTarget::ArenaA, DayPhase::BossFight);

        let context = app.world().resource::<LoadingContext>();
        assert_eq!(context.target, Some(LoadTarget::ArenaA));
        assert_eq!(context.resume_phase, Some(DayPhase::BossFight));
        assert!(!app.world().resource::<SceneFade>().is_idle());
    }

    #[test]
    fn a_second_request_is_ignored_while_a_fade_runs() {
        let mut app = setup_app();
        begin_load(&mut app, LoadTarget::ArenaA, DayPhase::BossFight);
        begin_load(&mut app, LoadTarget::Farm, DayPhase::Farming);

        let context = app.world().resource::<LoadingContext>();
        assert_eq!(
            context.target,
            Some(LoadTarget::ArenaA),
            "the first request wins"
        );
    }

    #[test]
    fn headless_game_transitions_are_immediate() {
        let mut app = setup_app();
        app.world_mut()
            .resource_mut::<SceneFade>()
            .begin_game(GameState::MainMenu);

        for _ in 0..3 {
            app.update();
        }

        assert_eq!(game(&app), GameState::MainMenu);
        assert!(app.world().resource::<SceneFade>().is_idle());
    }
}
