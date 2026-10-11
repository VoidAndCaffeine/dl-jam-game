//! The objective checklist: tracking, the on-screen goal list and its exit
//! animation.
//!
//! [`track_goals`] turns the game's existing messages (plant, harvest, craft,
//! boss defeat) into completed [`GoalId`]s and advances the tier chain. The goal
//! HUD, spawned under the top-right status column, shows the active tier's
//! outstanding goals; when one lands it is struck through, fades, then collapses
//! so the rows below slide up. The pause overlay reveals the same checklist for
//! every tier the player has reached (see [`crate::plugins::pause`]).

use crate::components::gear::GearSet;
use crate::events::{BossDefeated, CropHarvested, CropPlanted, CropWatered, GearCrafted};
use crate::resources::goals::{
    GoalId, GoalsProgress, description_for, goal_for_boss, goal_for_harvest, goal_for_plant,
};
use crate::resources::run_data::PlayerGear;
use crate::states::GameState;
use bevy::ecs::message::MessageReader;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

// --- Layout (authored at the UI reference height; `UiScale` handles the rest) --
const GOALS_RIGHT: f32 = 12.0;
const GOALS_TOP: f32 = 62.0;
const GOALS_HEADING_FONT: f32 = 19.5;
const GOAL_FONT: f32 = 15.0;
const GOAL_ROW_HEIGHT: f32 = 22.0;

// --- Exit animation timings ------------------------------------------------
/// How long the strikethrough sits before the row starts to fade.
const STRIKE_HOLD: f32 = 0.30;
const FADE_SECONDS: f32 = 0.50;
const COLLAPSE_SECONDS: f32 = 0.25;

// --- Palette ---------------------------------------------------------------
const GOAL_TEXT: Color = Color::srgb(0.90, 0.91, 0.96);
const GOAL_LINE: Color = Color::srgb(0.72, 0.74, 0.82);
const GOAL_DONE: Color = Color::srgb(0.45, 0.88, 0.55);
const HEADING: Color = Color::srgb(0.62, 0.63, 0.70);
const CLEAR: Color = Color::srgba(0.0, 0.0, 0.0, 0.0);

pub struct GoalsPlugin;

impl Plugin for GoalsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GoalsProgress>()
            .register_type::<GoalsProgress>()
            .register_type::<GoalId>()
            // Idempotent: the farm/gear/boss plugins already declared these, but
            // owning them here keeps the plugin usable on its own in tests.
            .add_message::<CropPlanted>()
            .add_message::<CropHarvested>()
            .add_message::<CropWatered>()
            .add_message::<GearCrafted>()
            .add_message::<BossDefeated>()
            .add_systems(OnEnter(GameState::Playing), spawn_goals_hud)
            .add_systems(OnExit(GameState::Playing), despawn_goals_hud)
            .add_systems(
                Update,
                // Chained so a goal completed this frame starts its animation
                // on the same frame.
                (track_goals, update_goals_hud)
                    .chain()
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

#[derive(Component, Debug, Default)]
pub struct GoalsHudRoot;

/// The column the goal rows are spawned into.
#[derive(Component, Debug, Default)]
pub struct GoalsList;

/// One visible goal and its exit-animation clock.
#[derive(Component, Debug, Clone, Copy)]
pub struct GoalRow {
    pub goal: GoalId,
}

#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub enum GoalRowAnim {
    Idle,
    Striking { elapsed: f32 },
    Fading { elapsed: f32 },
    Collapsing { elapsed: f32 },
}

#[derive(Component, Debug, Default)]
pub struct GoalRowLabel;

/// The thin line drawn over a completed goal's text.
#[derive(Component, Debug, Default)]
pub struct GoalStrike;

/// The small box beside a goal, filled when it is done.
#[derive(Component, Debug, Default)]
pub struct GoalCheckbox;

/// The row's text, strike line and checkbox colours, bundled so the update
/// system stays within the argument limit.
#[derive(SystemParam)]
pub struct GoalRowVisuals<'w, 's> {
    labels: Query<'w, 's, &'static mut TextColor, With<GoalRowLabel>>,
    strikes: Query<'w, 's, &'static mut BackgroundColor, (With<GoalStrike>, Without<GoalCheckbox>)>,
    checks: Query<'w, 's, &'static mut BackgroundColor, (With<GoalCheckbox>, Without<GoalStrike>)>,
}

impl GoalRowVisuals<'_, '_> {
    /// Paints the row as completed: the strike line appears and the checkbox
    /// fills. Called the frame the goal lands.
    fn mark_struck(&mut self, children: &Children) {
        for child in children.iter() {
            if let Ok(mut color) = self.labels.get_mut(child) {
                color.0 = GOAL_TEXT;
            }
            if let Ok(mut bg) = self.strikes.get_mut(child) {
                bg.0 = GOAL_LINE;
            }
            if let Ok(mut bg) = self.checks.get_mut(child) {
                bg.0 = GOAL_DONE;
            }
        }
    }

    /// Fades the whole row to `alpha`.
    fn fade(&mut self, children: &Children, alpha: f32) {
        for child in children.iter() {
            if let Ok(mut color) = self.labels.get_mut(child) {
                color.0 = with_alpha(GOAL_TEXT, alpha);
            }
            if let Ok(mut bg) = self.strikes.get_mut(child) {
                bg.0 = with_alpha(GOAL_LINE, alpha);
            }
            if let Ok(mut bg) = self.checks.get_mut(child) {
                bg.0 = with_alpha(GOAL_DONE, alpha);
            }
        }
    }
}

/// A colour with its alpha replaced (keeping a small testable pure helper).
fn with_alpha(color: Color, alpha: f32) -> Color {
    let c = color.to_srgba();
    Color::srgba(c.red, c.green, c.blue, alpha.clamp(0.0, 1.0))
}

fn spawn_goals_hud(mut commands: Commands) {
    commands
        .spawn((
            Name::new("Goals HUD"),
            GoalsHudRoot,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(GOALS_RIGHT),
                top: Val::Px(GOALS_TOP),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Px(3.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("GOALS"),
                TextFont {
                    font_size: FontSize::Px(GOALS_HEADING_FONT),
                    ..default()
                },
                TextColor(HEADING),
            ));
            panel.spawn((
                GoalsList,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexEnd,
                    row_gap: Val::Px(1.0),
                    ..default()
                },
            ));
        });
}

fn despawn_goals_hud(mut commands: Commands, roots: Query<Entity, With<GoalsHudRoot>>) {
    for entity in roots.iter() {
        commands.entity(entity).despawn();
    }
}

/// Turns the game's existing messages into completed goals, then releases the
/// next tier when the current one is finished.
fn track_goals(
    mut goals: ResMut<GoalsProgress>,
    gear: Res<PlayerGear>,
    mut planted: MessageReader<CropPlanted>,
    mut harvested: MessageReader<CropHarvested>,
    mut watered: MessageReader<CropWatered>,
    mut crafted: MessageReader<GearCrafted>,
    mut defeated: MessageReader<BossDefeated>,
) {
    for event in planted.read() {
        if let Some(goal) = goal_for_plant(event.0) {
            goals.complete(goal);
        }
    }
    for event in harvested.read() {
        if let Some(goal) = goal_for_harvest(event.crop) {
            goals.complete(goal);
        }
    }
    for _ in watered.read() {
        goals.complete(GoalId::WaterCrop);
    }
    for event in crafted.read() {
        let piece = event.0;
        // Any full set counts for the mid-game goal; the DreamLayer (Master) set
        // additionally ticks its own finale goal.
        if gear.owns_set(piece.set) {
            goals.complete(GoalId::CraftFullSet);
        }
        if piece.set == GearSet::Master && gear.owns_set(GearSet::Master) {
            goals.complete(GoalId::CraftDreamLayerSet);
        }
    }
    for event in defeated.read() {
        if let Some(goal) = goal_for_boss(event.0) {
            goals.complete(goal);
        }
    }
    goals.advance();
}

/// Reconciles the visible rows with the active tier and drives the exit
/// animation for goals that just completed.
fn update_goals_hud(
    mut commands: Commands,
    time: Res<Time>,
    goals: Res<GoalsProgress>,
    list: Query<Entity, With<GoalsList>>,
    mut rows: Query<(Entity, &GoalRow, &mut GoalRowAnim, &mut Node, &Children)>,
    mut visuals: GoalRowVisuals,
) {
    let dt = time.delta_secs();
    let mut finished: Vec<Entity> = Vec::new();

    for (entity, row, mut anim, mut node, children) in rows.iter_mut() {
        // A goal that just landed starts its exit animation.
        if *anim == GoalRowAnim::Idle && goals.is_completed(row.goal) {
            *anim = GoalRowAnim::Striking { elapsed: 0.0 };
            visuals.mark_struck(children);
        }
        match *anim {
            GoalRowAnim::Idle => {}
            GoalRowAnim::Striking { elapsed } => {
                let next = elapsed + dt;
                *anim = if next >= STRIKE_HOLD {
                    GoalRowAnim::Fading { elapsed: 0.0 }
                } else {
                    GoalRowAnim::Striking { elapsed: next }
                };
            }
            GoalRowAnim::Fading { elapsed } => {
                let next = elapsed + dt;
                let alpha = 1.0 - (next / FADE_SECONDS).clamp(0.0, 1.0);
                visuals.fade(children, alpha);
                *anim = if next >= FADE_SECONDS {
                    GoalRowAnim::Collapsing { elapsed: 0.0 }
                } else {
                    GoalRowAnim::Fading { elapsed: next }
                };
            }
            GoalRowAnim::Collapsing { elapsed } => {
                let next = elapsed + dt;
                let progress = (next / COLLAPSE_SECONDS).clamp(0.0, 1.0);
                node.height = Val::Px(GOAL_ROW_HEIGHT * (1.0 - progress));
                if next >= COLLAPSE_SECONDS {
                    finished.push(entity);
                } else {
                    *anim = GoalRowAnim::Collapsing { elapsed: next };
                }
            }
        }
    }

    for entity in finished {
        commands.entity(entity).despawn();
    }

    let Ok(list) = list.single() else {
        return;
    };
    let existing: Vec<GoalId> = rows.iter().map(|(_, row, _, _, _)| row.goal).collect();
    let missing: Vec<GoalId> = goals
        .active_goals()
        .into_iter()
        .filter(|goal| !existing.contains(goal))
        .collect();
    if missing.is_empty() {
        return;
    }
    commands.entity(list).with_children(|parent| {
        for goal in missing {
            spawn_goal_row(parent, goal);
        }
    });
}

fn spawn_goal_row(parent: &mut ChildSpawnerCommands, goal: GoalId) {
    parent
        .spawn((
            GoalRow { goal },
            GoalRowAnim::Idle,
            Node {
                height: Val::Px(GOAL_ROW_HEIGHT),
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                GoalCheckbox,
                Node {
                    width: Val::Px(9.0),
                    height: Val::Px(9.0),
                    flex_shrink: 0.0,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(CLEAR),
                BorderColor::all(GOAL_TEXT),
            ));
            row.spawn((
                GoalRowLabel,
                Text::new(description_for(goal)),
                TextFont {
                    font_size: FontSize::Px(GOAL_FONT),
                    ..default()
                },
                TextColor(GOAL_TEXT),
            ));
            // The strikethrough line, hidden until the goal completes.
            row.spawn((
                GoalStrike,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(18.0),
                    right: Val::Px(2.0),
                    top: Val::Px(GOAL_ROW_HEIGHT / 2.0 - 1.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(CLEAR),
            ));
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::gear::{GearPiece, GearSlot};
    use crate::components::pot::CropType;
    use bevy::state::app::StatesPlugin;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin, GoalsPlugin))
            .init_resource::<PlayerGear>()
            .init_state::<GameState>()
            // A fixed step so the exit animation advances deterministically.
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
                50,
            )));
        app
    }

    fn enter_playing(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        for _ in 0..2 {
            app.update();
        }
    }

    fn goals(app: &App) -> &GoalsProgress {
        app.world().resource::<GoalsProgress>()
    }

    fn row_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<GoalRow>>()
            .iter(app.world())
            .count()
    }

    /// Runs one frame; the manual time step above gives it a real delta.
    fn advance(app: &mut App) {
        app.update();
    }

    fn own_set(app: &mut App, set: GearSet) {
        let mut gear = app.world_mut().resource_mut::<PlayerGear>();
        for piece in set.pieces() {
            gear.own(piece);
        }
    }

    #[test]
    fn hud_spawns_on_enter_and_leaves_with_playing() {
        let mut app = setup_app();
        assert_eq!(row_count(&mut app), 0);

        enter_playing(&mut app);
        assert!(
            app.world_mut()
                .query_filtered::<Entity, With<GoalsHudRoot>>()
                .iter(app.world())
                .next()
                .is_some()
        );
        assert_eq!(row_count(&mut app), 2, "tier one shows its two goals");

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();
        app.update();

        assert!(
            app.world_mut()
                .query_filtered::<Entity, With<GoalsHudRoot>>()
                .iter(app.world())
                .next()
                .is_none()
        );
        assert_eq!(row_count(&mut app), 0);
    }

    #[test]
    fn planting_and_watering_finish_the_first_tier() {
        let mut app = setup_app();
        enter_playing(&mut app);

        app.world_mut()
            .write_message(CropPlanted(CropType::Starter));
        app.update();
        assert!(goals(&app).is_completed(GoalId::PlantStarter));
        assert_eq!(
            goals(&app).active_tier(),
            0,
            "watering is still outstanding"
        );

        app.world_mut().write_message(CropWatered);
        app.update();

        assert!(goals(&app).is_completed(GoalId::WaterCrop));
        assert_eq!(goals(&app).active_tier(), 1, "tier two is released");
        assert_eq!(
            goals(&app).active_goals(),
            vec![
                GoalId::HarvestStarter,
                GoalId::CraftFullSet,
                GoalId::BeatExcavator
            ]
        );
    }

    #[test]
    fn watering_a_crop_completes_the_water_goal() {
        let mut app = setup_app();
        enter_playing(&mut app);

        app.world_mut().write_message(CropWatered);
        app.update();

        assert!(goals(&app).is_completed(GoalId::WaterCrop));
    }

    #[test]
    fn harvesting_and_crafting_and_bossing_release_the_third_tier() {
        let mut app = setup_app();
        enter_playing(&mut app);
        // Jump to tier two: plant, then water.
        app.world_mut()
            .write_message(CropPlanted(CropType::Starter));
        app.world_mut().write_message(CropWatered);
        app.update();
        assert_eq!(goals(&app).active_tier(), 1);

        app.world_mut().write_message(CropHarvested {
            crop: CropType::Starter,
            position: Vec2::ZERO,
        });
        own_set(&mut app, GearSet::Starter);
        app.world_mut().write_message(GearCrafted(GearPiece::new(
            GearSet::Starter,
            GearSlot::Weapon,
        )));
        app.world_mut()
            .write_message(BossDefeated(crate::components::boss::BossId::BossA));
        app.update();

        assert!(goals(&app).is_completed(GoalId::HarvestStarter));
        assert!(goals(&app).is_completed(GoalId::WaterCrop));
        assert!(goals(&app).is_completed(GoalId::CraftFullSet));
        assert!(goals(&app).is_completed(GoalId::BeatExcavator));
        assert_eq!(goals(&app).active_tier(), 2);
    }

    #[test]
    fn any_full_set_satisfies_the_craft_goal() {
        for set in [GearSet::BossA, GearSet::BossB, GearSet::Master] {
            let mut app = setup_app();
            enter_playing(&mut app);
            own_set(&mut app, set);
            app.world_mut()
                .write_message(GearCrafted(GearPiece::new(set, GearSlot::Armor)));
            app.update();
            assert!(
                goals(&app).is_completed(GoalId::CraftFullSet),
                "{set:?} should complete the goal"
            );
        }
    }

    #[test]
    fn the_master_set_also_completes_the_dreamlayer_goal() {
        let mut app = setup_app();
        enter_playing(&mut app);
        own_set(&mut app, GearSet::Master);
        app.world_mut().write_message(GearCrafted(GearPiece::new(
            GearSet::Master,
            GearSlot::Armor,
        )));
        app.update();

        assert!(goals(&app).is_completed(GoalId::CraftDreamLayerSet));
    }

    #[test]
    fn a_completed_goal_row_animates_out_and_the_list_reflows() {
        let mut app = setup_app();
        enter_playing(&mut app);
        assert_eq!(row_count(&mut app), 2, "tier one shows its two goals");

        app.world_mut()
            .write_message(CropPlanted(CropType::Starter));
        app.update();

        // The same frame the goal lands, its row begins the exit animation.
        let striking = app
            .world_mut()
            .query_filtered::<&GoalRowAnim, With<GoalRow>>()
            .iter(app.world())
            .any(|anim| matches!(anim, GoalRowAnim::Striking { .. }));
        assert!(striking, "the planted goal's row strikes through");

        // Run the clock past strike + fade + collapse (50ms per frame).
        for _ in 0..40 {
            advance(&mut app);
        }

        // The struck row is gone; the outstanding "Water a Crop" row remains.
        assert_eq!(row_count(&mut app), 1);
        assert_eq!(goals(&app).active_goals(), vec![GoalId::WaterCrop]);
    }

    #[test]
    fn with_alpha_keeps_rgb_and_scales_alpha() {
        let faded = with_alpha(Color::srgb(0.5, 0.25, 1.0), 0.4).to_srgba();
        assert!((faded.red - 0.5).abs() < 1e-4);
        assert!((faded.green - 0.25).abs() < 1e-4);
        assert!((faded.blue - 1.0).abs() < 1e-4);
        assert!((faded.alpha - 0.4).abs() < 1e-4);
    }
}
