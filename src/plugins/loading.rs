use crate::resources::boss_sprite::BossSpriteAssets;
use crate::resources::effect_sprite::EffectSpriteAssets;
use crate::resources::player_sprite::PlayerSpriteAssets;
use crate::resources::run_data::PlayerGear;
use crate::resources::scene_assets::{LoadTarget, LoadingContext, SceneAssetManifest};
use crate::states::{DayPhase, GameState};
use bevy::asset::AssetServer;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

const BAR_WIDTH: f32 = 420.0;
const BAR_HEIGHT: f32 = 20.0;

const SCREEN_BG: Color = Color::srgba(0.03, 0.03, 0.05, 1.0);
const BAR_BG: Color = Color::srgb(0.16, 0.16, 0.21);
const BAR_FILL: Color = Color::srgb(0.44, 0.95, 0.52);
const BORDER: Color = Color::srgb(0.55, 0.55, 0.68);
const TEXT: Color = Color::srgb(0.94, 0.94, 0.97);

/// Preloads a scene's sprite sheets behind a progress bar.
///
/// The boot `GameState::LoadingAssets` state covers the first load from an
/// empty world; the in-game `DayPhase::Loading` state covers farm and arena
/// swaps without leaving `Playing` (which would reset the run). Both reuse the
/// same bar and the same [`SceneAssetManifest`].
pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SceneAssetManifest>()
            .init_resource::<LoadingContext>()
            .add_systems(
                OnEnter(GameState::LoadingAssets),
                (begin_boot_load, spawn_loading_screen).chain(),
            )
            .add_systems(OnExit(GameState::LoadingAssets), despawn_loading_screen)
            .add_systems(
                Update,
                poll_boot_load.run_if(in_state(GameState::LoadingAssets)),
            )
            .add_systems(
                OnEnter(DayPhase::Loading),
                (begin_scene_load, spawn_loading_screen).chain(),
            )
            .add_systems(
                OnExit(DayPhase::Loading),
                (clear_sprite_caches, despawn_loading_screen).chain(),
            )
            .add_systems(Update, poll_scene_load.run_if(in_state(DayPhase::Loading)))
            .add_systems(Update, update_loading_bar.run_if(loading_active));
    }
}

#[derive(Component, Reflect, Debug, Default)]
pub struct LoadingScreenRoot;

#[derive(Component, Reflect, Debug, Default)]
struct LoadingCamera;

#[derive(Component, Reflect, Debug, Default)]
struct LoadingBarFill;

/// True while either loading state is active. Reads the sub-state as optional so
/// it can be evaluated during the boot load, when `DayPhase` does not exist.
fn loading_active(game: Option<Res<State<GameState>>>, day: Option<Res<State<DayPhase>>>) -> bool {
    game.is_some_and(|state| state.get() == &GameState::LoadingAssets)
        || day.is_some_and(|state| state.get() == &DayPhase::Loading)
}

/// Queues the farm's sheets for the very first load.
fn begin_boot_load(
    mut manifest: ResMut<SceneAssetManifest>,
    mut context: ResMut<LoadingContext>,
    gear: Res<PlayerGear>,
    server: Option<Res<AssetServer>>,
) {
    let target = LoadTarget::Farm;
    context.target = Some(target);
    context.resume_phase = Some(DayPhase::Farming);
    if let Some(server) = server.as_deref() {
        manifest.begin(target, target.required_paths(armor_set(&gear)), server);
    }
}

/// Queues the sheets the pending [`LoadingContext`] target needs.
fn begin_scene_load(
    mut manifest: ResMut<SceneAssetManifest>,
    context: Res<LoadingContext>,
    gear: Res<PlayerGear>,
    server: Option<Res<AssetServer>>,
) {
    let Some(target) = context.target else {
        return;
    };
    if let Some(server) = server.as_deref() {
        manifest.begin(target, target.required_paths(armor_set(&gear)), server);
    }
}

fn armor_set(gear: &PlayerGear) -> Option<crate::components::gear::GearSet> {
    gear.armor.map(|piece| piece.set)
}

/// Boot is done when the farm's sheets have settled; then the title screen
/// takes over. `New Game` moves on to `Playing` (the farm is already resident).
fn poll_boot_load(
    manifest: Res<SceneAssetManifest>,
    server: Option<Res<AssetServer>>,
    mut next_game: ResMut<NextState<GameState>>,
) {
    if manifest.is_complete(server.as_deref()) {
        next_game.set(GameState::MainMenu);
    }
}

/// Moves into the phase the loading was started for once the scene is resident.
fn poll_scene_load(
    manifest: Res<SceneAssetManifest>,
    context: Res<LoadingContext>,
    server: Option<Res<AssetServer>>,
    mut next_phase: ResMut<NextState<DayPhase>>,
) {
    if !manifest.is_complete(server.as_deref()) {
        return;
    }
    if let Some(phase) = context.resume_phase.clone() {
        next_phase.set(phase);
    }
}

/// The lazy caches hold their own strong handles, so they have to forget the
/// old scene or nothing behind the manifest could ever unload. Both are owned by
/// other plugins, so they are optional here.
fn clear_sprite_caches(
    mut player: Option<ResMut<PlayerSpriteAssets>>,
    mut boss: Option<ResMut<BossSpriteAssets>>,
    mut effect: Option<ResMut<EffectSpriteAssets>>,
) {
    if let Some(player) = player.as_deref_mut() {
        player.clear();
    }
    if let Some(boss) = boss.as_deref_mut() {
        boss.clear();
    }
    if let Some(effect) = effect.as_deref_mut() {
        effect.clear();
    }
}

fn spawn_loading_screen(
    mut commands: Commands,
    manifest: Res<SceneAssetManifest>,
    cameras: Query<(), With<Camera2d>>,
) {
    // The boot load runs before the game camera exists, so UI has nothing to
    // draw to without a temporary one.
    if cameras.is_empty() {
        commands.spawn((Name::new("Loading Camera"), LoadingCamera, Camera2d));
    }

    let title = manifest.target().map_or("Loading", LoadTarget::label);
    commands
        .spawn((
            Name::new("Loading Screen"),
            LoadingScreenRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(18.0),
                ..default()
            },
            BackgroundColor(SCREEN_BG),
            GlobalZIndex(1000),
            FocusPolicy::Block,
        ))
        .with_children(|screen| {
            screen.spawn((
                Text::new(format!("Loading {title}…")),
                TextFont {
                    font_size: FontSize::Px(26.0),
                    ..default()
                },
                TextColor(TEXT),
            ));
            screen
                .spawn((
                    Name::new("Loading Bar"),
                    Node {
                        width: Val::Px(BAR_WIDTH),
                        height: Val::Px(BAR_HEIGHT),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(BAR_BG),
                    BorderColor::all(BORDER),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        LoadingBarFill,
                        Node {
                            width: Val::Percent(0.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(BAR_FILL),
                    ));
                });
        });
}

fn despawn_loading_screen(
    mut commands: Commands,
    roots: Query<Entity, With<LoadingScreenRoot>>,
    cameras: Query<Entity, With<LoadingCamera>>,
) {
    for entity in roots.iter().chain(cameras.iter()) {
        commands.entity(entity).despawn();
    }
}

fn update_loading_bar(
    manifest: Res<SceneAssetManifest>,
    server: Option<Res<AssetServer>>,
    mut fills: Query<&mut Node, With<LoadingBarFill>>,
) {
    let Some(server) = server else {
        return;
    };
    let percent = manifest.progress(&server) * 100.0;
    for mut node in fills.iter_mut() {
        node.width = Val::Percent(percent);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin, LoadingPlugin))
            .init_resource::<PlayerGear>()
            .init_state::<GameState>()
            .init_state::<DayPhase>();
        app
    }

    fn roots(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<LoadingScreenRoot>>()
            .iter(app.world())
            .count()
    }

    fn phase(app: &App) -> DayPhase {
        app.world().resource::<State<DayPhase>>().get().clone()
    }

    fn game_state(app: &App) -> GameState {
        app.world().resource::<State<GameState>>().get().clone()
    }

    fn settle(app: &mut App, frames: usize) {
        for _ in 0..frames {
            app.update();
        }
    }

    #[test]
    fn boot_shows_a_loading_screen_then_enters_the_title_screen() {
        let mut app = setup_app();
        // The default GameState is LoadingAssets, so the first update enters it.
        app.update();
        assert_eq!(roots(&mut app), 1, "the bar is up while loading");
        assert_eq!(game_state(&app), GameState::LoadingAssets);

        settle(&mut app, 3);
        assert_eq!(game_state(&app), GameState::MainMenu);
        assert_eq!(roots(&mut app), 0, "the bar is torn down on entry");
    }

    #[test]
    fn an_in_game_target_resumes_its_phase() {
        let mut app = setup_app();
        settle(&mut app, 3);
        assert_eq!(game_state(&app), GameState::MainMenu);

        // Starting a run drops into Playing, where the day phases exist.
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        assert_eq!(game_state(&app), GameState::Playing);

        {
            let mut context = app.world_mut().resource_mut::<LoadingContext>();
            context.target = Some(LoadTarget::ArenaA);
            context.resume_phase = Some(DayPhase::BossFight);
        }
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Loading);
        app.update();

        assert_eq!(phase(&app), DayPhase::Loading);
        assert_eq!(roots(&mut app), 1);

        settle(&mut app, 3);
        assert_eq!(phase(&app), DayPhase::BossFight);
        assert_eq!(roots(&mut app), 0);
    }

    #[test]
    fn a_boot_camera_is_only_added_when_none_exists() {
        let mut app = setup_app();
        app.world_mut().spawn(Camera2d);
        app.update();

        let cameras = app
            .world_mut()
            .query_filtered::<Entity, With<LoadingCamera>>()
            .iter(app.world())
            .count();
        assert_eq!(cameras, 0, "the existing camera is reused");
    }
}
