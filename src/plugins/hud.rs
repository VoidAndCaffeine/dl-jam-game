use crate::components::boss::{Boss, BossSpawnMarker};
use crate::components::player::{Health, Player};
use crate::resources::farm::DayCounter;
use crate::resources::player_status::PlayerStatus;
use crate::states::{GameState, Phase};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

const BAR_WIDTH: f32 = 200.0;
const BAR_HEIGHT: f32 = 16.0;
const HUD_PADDING: f32 = 12.0;

/// How quickly the lagging (yellow) fill catches up to the live (red) fill.
const HEALTH_LERP_RATE: f32 = 8.0;

const BAR_BG: Color = Color::srgb(0.20, 0.20, 0.25);
const BAR_FILL: Color = Color::srgb(0.80, 0.10, 0.10);
const BAR_LAG: Color = Color::srgb(1.0, 0.80, 0.10);
const TEXT_PRIMARY: Color = Color::srgb(0.94, 0.94, 0.97);
const TEXT_DIM: Color = Color::srgb(0.62, 0.63, 0.70);

const FARMING_HINT: &str =
    "WASD Move   |   Space / Click Interact   |   Tab Craft   |   I Inventory";
const BOSS_SELECT_HINT: &str = "Arrows Select   |   Enter / Space Choose   |   Esc Back";
const BOSS_FIGHT_HINT: &str =
    "WASD Move   |   Q / LMB Light   |   E / RMB Heavy   |   Z / X Lock   |   I Inventory";
/// Shown in place of [`BOSS_FIGHT_HINT`] while madness reverses the controls,
/// spelling out the swapped left/right keys. ASCII only: the default font has no
/// symbol glyphs.
const INVERTED_HINT: &str = "CONTROLS INVERTED   |   D = Left   A = Right   |   Q / LMB Light   |   E / RMB Heavy   |   Z / X Lock   |   I Inventory";
const RESULT_HINT: &str = "Any key to continue";

/// The red the hint text flashes to while the controls are reversed. It eases
/// back to [`TEXT_DIM`] and forth again once per [`HINT_PULSE_PERIOD`].
const HINT_RED: Color = Color::srgb(1.0, 0.15, 0.15);
/// Seconds for one full red-to-dim-to-red pulse of the hint text.
const HINT_PULSE_PERIOD: f32 = 0.5;

/// Spawns the whole HUD (player health, boss health, day, hints) when play
/// begins, and tears it back down on the way out.
pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Playing), spawn_hud)
            .add_systems(OnExit(GameState::Playing), despawn_hud)
            .add_systems(
                Update,
                (
                    update_player_health,
                    update_boss_health,
                    update_day_text,
                    update_hints,
                    pulse_hints,
                )
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

#[derive(Component, Reflect, Debug, Default)]
pub struct HudRoot;

#[derive(Component, Reflect, Debug, Default)]
pub struct PlayerHealthGroup;

#[derive(Component, Reflect, Debug, Default)]
pub struct BossHealthGroup;

#[derive(Component, Reflect, Debug, Default)]
pub struct DayText;

#[derive(Component, Reflect, Debug, Default)]
pub struct HintsText;

/// Drives the red pulse on the hint text while madness reverses the controls.
/// `elapsed` is the time within the current pulse, reset when the effect lapses.
#[derive(Component, Reflect, Debug, Default)]
pub struct HintsPulse {
    pub elapsed: f32,
}

#[derive(Component, Reflect, Debug, Default)]
pub struct BossNameText;

/// The live state of one health bar. `target` drives the red fill instantly,
/// `display` trails it for the yellow damage flash.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct HealthBar {
    pub target: f32,
    pub display: f32,
}

impl Default for HealthBar {
    fn default() -> Self {
        Self {
            target: 1.0,
            display: 1.0,
        }
    }
}

#[derive(Component, Reflect, Debug, Default)]
pub struct PlayerHealthBar;

#[derive(Component, Reflect, Debug, Default)]
pub struct BossHealthBar;

#[derive(Component, Reflect, Debug, Default)]
pub struct HealthBarRedFill;

#[derive(Component, Reflect, Debug, Default)]
pub struct HealthBarYellowFill;

type RedFills<'w, 's> =
    Query<'w, 's, &'static mut Node, (With<HealthBarRedFill>, Without<HealthBarYellowFill>)>;

type YellowFills<'w, 's> =
    Query<'w, 's, &'static mut Node, (With<HealthBarYellowFill>, Without<HealthBarRedFill>)>;

/// The two fill layers of any health bar, bundled so bar update systems stay
/// within the argument limit.
#[derive(SystemParam)]
pub struct HealthBarFills<'w, 's> {
    red: RedFills<'w, 's>,
    yellow: YellowFills<'w, 's>,
}

fn label(text: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(color),
    )
}

/// A grey 200px bar with a yellow trailing fill behind an instant red fill.
fn spawn_health_bar(parent: &mut ChildSpawnerCommands, marker: impl Component) {
    parent
        .spawn((
            marker,
            HealthBar::default(),
            Node {
                width: Val::Px(BAR_WIDTH),
                height: Val::Px(BAR_HEIGHT),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(BAR_BG),
            BorderColor::all(TEXT_DIM),
        ))
        .with_children(|bar| {
            bar.spawn((
                HealthBarYellowFill,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(BAR_LAG),
            ));
            bar.spawn((
                HealthBarRedFill,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(BAR_FILL),
            ));
        });
}

fn spawn_hud(mut commands: Commands) {
    commands
        .spawn((
            Name::new("HUD"),
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .with_children(|hud| {
            // Player health - top-left.
            hud.spawn((
                Name::new("Player Health"),
                PlayerHealthGroup,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(HUD_PADDING),
                    top: Val::Px(HUD_PADDING),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    ..default()
                },
            ))
            .with_children(|group| {
                group.spawn(label("HP", 16.0, TEXT_DIM));
                spawn_health_bar(group, PlayerHealthBar);
            });

            // Boss health - top-center, hidden until a fight is live.
            hud.spawn((
                Name::new("Boss Health"),
                BossHealthGroup,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    top: Val::Px(HUD_PADDING),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    ..default()
                },
                Visibility::Hidden,
            ))
            .with_children(|group| {
                group.spawn((BossNameText, label("", 16.0, TEXT_PRIMARY)));
                spawn_health_bar(group, BossHealthBar);
            });

            // Day counter - top-right.
            hud.spawn((
                Name::new("Day"),
                DayText,
                label("DAY 1", 20.0, TEXT_PRIMARY),
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(HUD_PADDING),
                    top: Val::Px(HUD_PADDING),
                    ..default()
                },
            ));

            // Contextual hints - bottom-center.
            hud.spawn((
                Name::new("Hints"),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    right: Val::Px(0.0),
                    bottom: Val::Px(HUD_PADDING),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
            ))
            .with_children(|hints| {
                hints.spawn((HintsText, HintsPulse::default(), label("", 15.0, TEXT_DIM)));
            });
        });
}

fn despawn_hud(mut commands: Commands, roots: Query<Entity, With<HudRoot>>) {
    for entity in roots.iter() {
        commands.entity(entity).despawn();
    }
}

fn update_player_health(
    time: Res<Time>,
    players: Query<&Health, With<Player>>,
    mut bars: Query<(&mut HealthBar, &Children), With<PlayerHealthBar>>,
    mut fills: HealthBarFills,
) {
    let fraction = players.iter().map(Health::fraction).next().unwrap_or(1.0);
    let dt = time.delta_secs();
    for (mut bar, children) in bars.iter_mut() {
        apply_bar(&mut bar, children, fraction, dt, &mut fills);
    }
}

fn update_boss_health(
    time: Res<Time>,
    phase: Phase,
    bosses: Query<&Boss, With<BossSpawnMarker>>,
    mut groups: Query<&mut Visibility, With<BossHealthGroup>>,
    mut names: Query<&mut Text, With<BossNameText>>,
    mut bars: Query<(&mut HealthBar, &Children), With<BossHealthBar>>,
    mut fills: HealthBarFills,
) {
    let show = phase.is_boss_fight() && !bosses.is_empty();
    for mut visibility in groups.iter_mut() {
        *visibility = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if !show {
        return;
    }

    let mut current = 0.0;
    let mut max = 0.0;
    let mut name = String::new();
    for boss in bosses.iter() {
        current += boss.health;
        max += boss.max_health;
        if name.is_empty() {
            name = boss.id.label().to_string();
        }
    }
    let fraction = if max > 0.0 {
        (current / max).clamp(0.0, 1.0)
    } else {
        0.0
    };

    for mut text in names.iter_mut() {
        if text.0 != name {
            text.0 = name.clone();
        }
    }

    let dt = time.delta_secs();
    for (mut bar, children) in bars.iter_mut() {
        apply_bar(&mut bar, children, fraction, dt, &mut fills);
    }
}

fn update_day_text(day: Res<DayCounter>, mut texts: Query<&mut Text, With<DayText>>) {
    let value = format!("DAY {}", day.0.max(1));
    for mut text in texts.iter_mut() {
        if text.0 != value {
            text.0 = value.clone();
        }
    }
}

fn update_hints(
    phase: Phase,
    status: Res<PlayerStatus>,
    mut texts: Query<&mut Text, With<HintsText>>,
) {
    let hint = if phase.is_boss_fight() && status.is_reversed() {
        INVERTED_HINT
    } else if phase.is_farming() {
        FARMING_HINT
    } else if phase.is_boss_select() {
        BOSS_SELECT_HINT
    } else if phase.is_boss_fight() {
        BOSS_FIGHT_HINT
    } else if phase.is_result() {
        RESULT_HINT
    } else {
        ""
    };
    for mut text in texts.iter_mut() {
        if text.0 != hint {
            text.0 = hint.to_string();
        }
    }
}

/// Flashes the hint text red and back while madness reverses the controls,
/// settling it to the normal dim colour once the effect lapses.
fn pulse_hints(
    time: Res<Time>,
    status: Res<PlayerStatus>,
    mut hints: Query<(&mut TextColor, &mut HintsPulse), With<HintsText>>,
) {
    let reversed = status.is_reversed();
    for (mut color, mut pulse) in hints.iter_mut() {
        if reversed {
            pulse.elapsed += time.delta_secs();
            color.0 = hint_pulse_color(pulse.elapsed);
        } else if pulse.elapsed != 0.0 {
            pulse.elapsed = 0.0;
            color.0 = TEXT_DIM;
        }
    }
}

/// A red-to-dim colour for the hint pulse, starting at full red on the frame
/// the reversal lands so the change reads as a flash.
fn hint_pulse_color(elapsed: f32) -> Color {
    let blend = (elapsed / HINT_PULSE_PERIOD * std::f32::consts::TAU).cos() * 0.5 + 0.5;
    let dim = TEXT_DIM.to_srgba();
    let red = HINT_RED.to_srgba();
    Color::srgb(
        dim.red + (red.red - dim.red) * blend,
        dim.green + (red.green - dim.green) * blend,
        dim.blue + (red.blue - dim.blue) * blend,
    )
}

/// Advances one bar's red fill instantly and eases its yellow fill after it.
fn apply_bar(
    bar: &mut HealthBar,
    children: &Children,
    target: f32,
    dt: f32,
    fills: &mut HealthBarFills<'_, '_>,
) {
    bar.target = target.clamp(0.0, 1.0);
    bar.display = lag_towards(bar.display, bar.target, dt);

    for child in children.iter() {
        if let Ok(mut node) = fills.red.get_mut(child) {
            node.width = Val::Percent(bar.target * 100.0);
        }
        if let Ok(mut node) = fills.yellow.get_mut(child) {
            node.width = Val::Percent(bar.display * 100.0);
        }
    }
}

/// Exponential ease from `display` toward `target`, frame-rate independent.
fn lag_towards(display: f32, target: f32, dt: f32) -> f32 {
    let blend = 1.0 - (-HEALTH_LERP_RATE * dt).exp();
    let next = display + (target - display) * blend;
    if (next - target).abs() < 0.001 {
        target
    } else {
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::boss::BossId;
    use crate::states::DayPhase;
    use bevy::state::app::StatesPlugin;
    use std::time::Duration;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin, HudPlugin))
            .init_resource::<Time>()
            .init_resource::<DayCounter>()
            .init_resource::<PlayerStatus>()
            .init_state::<GameState>()
            .init_state::<DayPhase>();
        app
    }

    fn enter_playing(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        app.update();
    }

    fn set_phase(app: &mut App, phase: DayPhase) {
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(phase);
        tick(app);
    }

    /// Advances the clock then runs a frame, so timed systems see a real delta.
    fn tick(app: &mut App) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(16));
        app.update();
    }

    fn hud_roots(app: &mut App) -> Vec<Entity> {
        app.world_mut()
            .query_filtered::<Entity, With<HudRoot>>()
            .iter(app.world())
            .collect()
    }

    /// Returns (red, yellow) widths for the bar with the given marker.
    fn bar_widths<M: Component>(app: &mut App) -> (Val, Val) {
        let container = app
            .world_mut()
            .query_filtered::<Entity, With<M>>()
            .iter(app.world())
            .next()
            .expect("bar container exists");
        let children: Vec<Entity> = app
            .world()
            .get::<Children>(container)
            .expect("container has children")
            .iter()
            .collect();
        let mut red = Val::Auto;
        let mut yellow = Val::Auto;
        for child in children {
            if app.world().get::<HealthBarRedFill>(child).is_some() {
                red = app.world().get::<Node>(child).unwrap().width;
            }
            if app.world().get::<HealthBarYellowFill>(child).is_some() {
                yellow = app.world().get::<Node>(child).unwrap().width;
            }
        }
        (red, yellow)
    }

    fn boss_group_visibility(app: &mut App) -> Visibility {
        *app.world_mut()
            .query_filtered::<&Visibility, With<BossHealthGroup>>()
            .iter(app.world())
            .next()
            .expect("boss group exists")
    }

    fn boss_name(app: &mut App) -> String {
        app.world_mut()
            .query_filtered::<&Text, With<BossNameText>>()
            .iter(app.world())
            .next()
            .map(|text| text.0.clone())
            .expect("boss name exists")
    }

    fn day_text(app: &mut App) -> String {
        app.world_mut()
            .query_filtered::<&Text, With<DayText>>()
            .iter(app.world())
            .next()
            .map(|text| text.0.clone())
            .expect("day text exists")
    }

    fn hints_text(app: &mut App) -> String {
        app.world_mut()
            .query_filtered::<&Text, With<HintsText>>()
            .iter(app.world())
            .next()
            .map(|text| text.0.clone())
            .expect("hints text exists")
    }

    fn hints_color(app: &mut App) -> Color {
        app.world_mut()
            .query_filtered::<&TextColor, With<HintsText>>()
            .iter(app.world())
            .next()
            .map(|color| color.0)
            .expect("hints text exists")
    }

    fn reverse(app: &mut App, seconds: f32) {
        app.world_mut()
            .resource_mut::<PlayerStatus>()
            .apply_reversal(seconds);
    }

    /// Advances the clock by `millis` then runs a frame.
    fn advance(app: &mut App, millis: u64) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(millis));
        app.update();
    }

    #[test]
    fn hud_spawns_on_enter_playing() {
        let mut app = setup_app();
        assert!(hud_roots(&mut app).is_empty());

        enter_playing(&mut app);

        assert_eq!(hud_roots(&mut app).len(), 1);
        assert!(
            app.world_mut()
                .query_filtered::<Entity, With<PlayerHealthBar>>()
                .iter(app.world())
                .next()
                .is_some()
        );
    }

    #[test]
    fn hud_despawns_on_exit_playing() {
        let mut app = setup_app();
        enter_playing(&mut app);
        assert_eq!(hud_roots(&mut app).len(), 1);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();
        app.update();

        assert!(hud_roots(&mut app).is_empty());
    }

    #[test]
    fn player_red_fill_updates_instantly_while_yellow_lags() {
        let mut app = setup_app();
        enter_playing(&mut app);
        let player = app
            .world_mut()
            .spawn((Player, Health::new(100.0, 0.0)))
            .id();
        tick(&mut app);

        // Damage to half.
        app.world_mut().get_mut::<Health>(player).unwrap().current = 50.0;
        for _ in 0..5 {
            tick(&mut app);
        }

        let (red, yellow) = bar_widths::<PlayerHealthBar>(&mut app);
        assert_eq!(red, Val::Percent(50.0), "red fill is instant");
        assert!(
            matches!(yellow, Val::Percent(value) if value > 50.0 && value < 100.0),
            "yellow eases down from full, at {yellow:?}"
        );
    }

    #[test]
    fn lag_towards_eases_to_the_target_without_overshooting() {
        let mut value = 1.0;
        for _ in 0..200 {
            value = lag_towards(value, 0.5, 1.0 / 60.0);
            assert!(value >= 0.5, "never dips past the target, got {value}");
            assert!(value <= 1.0);
        }
        assert!(
            (value - 0.5).abs() < 0.001,
            "settles on the target: {value}"
        );

        // A zero-length frame changes nothing.
        assert_eq!(lag_towards(0.8, 0.2, 0.0), 0.8);
    }

    #[test]
    fn boss_bar_is_hidden_outside_a_fight() {
        let mut app = setup_app();
        enter_playing(&mut app);
        assert_eq!(boss_group_visibility(&mut app), Visibility::Hidden);

        set_phase(&mut app, DayPhase::BossSelect);
        assert_eq!(boss_group_visibility(&mut app), Visibility::Hidden);
    }

    #[test]
    fn boss_bar_shows_combined_dual_health_during_a_fight() {
        let mut app = setup_app();
        enter_playing(&mut app);
        set_phase(&mut app, DayPhase::BossFight);

        for _ in 0..2 {
            let mut boss = Boss::new(BossId::Dual);
            boss.health = boss.max_health * 0.25;
            app.world_mut()
                .spawn((boss, BossSpawnMarker, Transform::default()));
        }
        tick(&mut app);

        assert_eq!(boss_group_visibility(&mut app), Visibility::Inherited);
        let (red, _) = bar_widths::<BossHealthBar>(&mut app);
        assert_eq!(red, Val::Percent(25.0), "dual halves share one pool");
        assert_eq!(boss_name(&mut app), "The Excavator & Mercurial");
    }

    #[test]
    fn day_text_tracks_the_day_counter() {
        let mut app = setup_app();
        enter_playing(&mut app);
        app.world_mut().insert_resource(DayCounter(4));
        tick(&mut app);

        assert_eq!(day_text(&mut app), "DAY 4");
    }

    #[test]
    fn hints_follow_the_current_phase() {
        let mut app = setup_app();
        enter_playing(&mut app);
        assert_eq!(hints_text(&mut app), FARMING_HINT);

        set_phase(&mut app, DayPhase::BossSelect);
        assert_eq!(hints_text(&mut app), BOSS_SELECT_HINT);

        set_phase(&mut app, DayPhase::BossFight);
        assert_eq!(hints_text(&mut app), BOSS_FIGHT_HINT);

        set_phase(&mut app, DayPhase::Result);
        assert_eq!(hints_text(&mut app), RESULT_HINT);
    }

    #[test]
    fn hints_show_the_inverted_banner_while_reversed() {
        let mut app = setup_app();
        enter_playing(&mut app);
        set_phase(&mut app, DayPhase::BossFight);
        assert_eq!(hints_text(&mut app), BOSS_FIGHT_HINT);

        reverse(&mut app, 2.0);
        tick(&mut app);
        assert_eq!(hints_text(&mut app), INVERTED_HINT);

        app.world_mut().resource_mut::<PlayerStatus>().clear();
        tick(&mut app);
        assert_eq!(hints_text(&mut app), BOSS_FIGHT_HINT);
    }

    #[test]
    fn hint_text_pulses_red_while_reversed() {
        let mut app = setup_app();
        enter_playing(&mut app);
        set_phase(&mut app, DayPhase::BossFight);
        reverse(&mut app, 10.0);

        // The frame the reversal lands is the brightest: nearly full red.
        tick(&mut app);
        let bright = hints_color(&mut app).to_srgba();
        assert!(
            bright.red > 0.95 && bright.green < 0.2,
            "starts red, got {bright:?}"
        );

        // Half a pulse later it has eased back toward the dim colour.
        advance(&mut app, 250);
        let dim = hints_color(&mut app).to_srgba();
        assert!(
            dim.green > bright.green,
            "pulses back toward dim, got {dim:?}"
        );
    }

    #[test]
    fn hint_colour_returns_to_dim_when_the_reversal_expires() {
        let mut app = setup_app();
        enter_playing(&mut app);
        set_phase(&mut app, DayPhase::BossFight);
        reverse(&mut app, 10.0);
        tick(&mut app);
        assert_ne!(hints_color(&mut app), TEXT_DIM);

        app.world_mut().resource_mut::<PlayerStatus>().clear();
        tick(&mut app);
        assert_eq!(hints_color(&mut app), TEXT_DIM);
    }

    #[test]
    fn pulse_starts_at_full_red_and_returns_to_dim() {
        let start = hint_pulse_color(0.0).to_srgba();
        let red = HINT_RED.to_srgba();
        assert!((start.red - red.red).abs() < 0.001, "starts red: {start:?}");
        assert!((start.green - red.green).abs() < 0.001);

        let half = hint_pulse_color(HINT_PULSE_PERIOD * 0.5).to_srgba();
        let dim = TEXT_DIM.to_srgba();
        assert!(
            (half.red - dim.red).abs() < 0.001,
            "dims mid-pulse: {half:?}"
        );
        assert!((half.green - dim.green).abs() < 0.001);
    }
}
