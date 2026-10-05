use crate::components::boss::BossId;
use crate::components::gear::MaterialType;
use crate::events::{BossDefeated, PlayerDied};
use crate::levels::LevelId;
use crate::resources::boss_progress::BossProgress;
use crate::resources::day_cycle::{DayCycle, Outcome};
use crate::resources::farm::CropUnlocks;
use crate::resources::inventory::Inventory;
use crate::resources::level::LevelRequest;
use crate::states::{DayPhase, GameState, Phase};
use bevy::ecs::message::MessageReader;
use bevy::prelude::*;

/// How much material a boss drops on defeat.
pub const BOSS_MATERIAL_DROP: u32 = 3;

pub struct DayCyclePlugin;

impl Plugin for DayCyclePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DayCycle>()
            .init_resource::<BossProgress>()
            .add_message::<BossDefeated>()
            .add_message::<PlayerDied>()
            .add_systems(OnEnter(GameState::Playing), begin_first_day)
            .add_systems(Update, on_boss_defeated)
            .add_systems(Update, on_player_died)
            .add_systems(OnEnter(DayPhase::Result), spawn_result_screen)
            .add_systems(OnExit(DayPhase::Result), despawn_result_screen)
            .add_systems(Update, result_continue);
    }
}

/// The first day works like any other: one pending advance when play starts.
fn begin_first_day(mut day_cycle: ResMut<DayCycle>) {
    day_cycle.request_advance();
}

/// Grants the boss's rewards, marks progress, then shows the result.
fn on_boss_defeated(
    mut events: MessageReader<BossDefeated>,
    mut progress: ResMut<BossProgress>,
    mut unlocks: ResMut<CropUnlocks>,
    mut inventory: ResMut<Inventory>,
    mut day_cycle: ResMut<DayCycle>,
    mut next_phase: ResMut<NextState<DayPhase>>,
) {
    for event in events.read() {
        let id = event.0;
        progress.record(id);
        match id {
            BossId::BossA => {
                unlocks.unlock_crop_a();
                inventory.add_material(MaterialType::BossA, BOSS_MATERIAL_DROP);
            }
            BossId::BossB => {
                unlocks.unlock_crop_b();
                inventory.add_material(MaterialType::BossB, BOSS_MATERIAL_DROP);
            }
            BossId::Dual => {
                day_cycle.run_complete = true;
            }
        }
        day_cycle.finish(Outcome::Victory);
        next_phase.set(DayPhase::Result);
    }
}

fn on_player_died(
    mut events: MessageReader<PlayerDied>,
    mut day_cycle: ResMut<DayCycle>,
    mut next_phase: ResMut<NextState<DayPhase>>,
) {
    for _ in events.read() {
        day_cycle.finish(Outcome::Defeat);
        day_cycle.run_complete = false;
        next_phase.set(DayPhase::Result);
    }
}

/// The result screen waits for a key, then starts the next day (or ends the run
/// once the dual boss is down).
fn result_continue(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut day_cycle: ResMut<DayCycle>,
    mut request: ResMut<LevelRequest>,
    mut next_phase: ResMut<NextState<DayPhase>>,
    mut next_game: ResMut<NextState<GameState>>,
    phase: Phase,
) {
    if !phase.is_result() {
        return;
    }
    let any_key = keys.get_just_pressed().next().is_some() || mouse.just_pressed(MouseButton::Left);
    if !any_key {
        return;
    }

    if day_cycle.run_complete {
        next_game.set(GameState::Victory);
        return;
    }
    day_cycle.request_advance();
    request.0 = Some(LevelId::Farm);
    next_phase.set(DayPhase::Farming);
}

#[derive(Component, Reflect, Default, Debug)]
pub struct ResultScreenRoot;

fn spawn_result_screen(mut commands: Commands, day_cycle: Res<DayCycle>) {
    let title = day_cycle.outcome.unwrap_or_default().title();
    commands
        .spawn((
            Name::new("Result Screen"),
            ResultScreenRoot,
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
            BackgroundColor(Color::srgba(0.03, 0.03, 0.05, 0.85)),
        ))
        .with_children(|screen| {
            screen.spawn((
                Text::new(title),
                TextFont {
                    font_size: FontSize::Px(64.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
            screen.spawn((
                Text::new("Press any key to continue"),
                TextFont {
                    font_size: FontSize::Px(22.0),
                    ..default()
                },
                TextColor(Color::srgb(0.8, 0.8, 0.86)),
            ));
        });
}

fn despawn_result_screen(mut commands: Commands, screens: Query<Entity, With<ResultScreenRoot>>) {
    for entity in screens.iter() {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::boss::BossId;
    use crate::components::pot::CropType;
    use crate::resources::boss_progress::BossProgress;
    use crate::resources::farm::CropUnlocks;
    use crate::resources::inventory::Inventory;
    use crate::resources::level::LevelRequest;
    use bevy::state::app::StatesPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<DayCycle>()
            .init_resource::<BossProgress>()
            .init_resource::<CropUnlocks>()
            .init_resource::<Inventory>()
            .init_resource::<LevelRequest>()
            .add_plugins((MinimalPlugins, StatesPlugin, DayCyclePlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>();

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);
        app.update();
        app
    }

    fn set_phase(app: &mut App, day: DayPhase) {
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(day);
        app.update();
    }

    fn day_cycle(app: &App) -> &DayCycle {
        app.world().resource::<DayCycle>()
    }

    fn progress(app: &App) -> &BossProgress {
        app.world().resource::<BossProgress>()
    }

    #[test]
    fn defeating_boss_a_unlocks_crop_a_and_drops_material() {
        let mut app = setup_app();
        set_phase(&mut app, DayPhase::BossFight);

        app.world_mut().write_message(BossDefeated(BossId::BossA));
        app.update();
        app.update();

        assert!(
            app.world()
                .resource::<CropUnlocks>()
                .is_unlocked(CropType::CropA)
        );
        assert_eq!(
            app.world()
                .resource::<Inventory>()
                .material_count(MaterialType::BossA),
            BOSS_MATERIAL_DROP
        );
        assert!(progress(&app).boss_a);
        assert_eq!(day_cycle(&app).outcome, Some(Outcome::Victory));
        assert_eq!(
            app.world().resource::<State<DayPhase>>().get(),
            &DayPhase::Result
        );
    }

    #[test]
    fn defeating_both_bosses_unlocks_the_dual_boss() {
        let mut app = setup_app();
        set_phase(&mut app, DayPhase::BossFight);
        app.world_mut().write_message(BossDefeated(BossId::BossA));
        app.update();
        set_phase(&mut app, DayPhase::BossFight);
        app.world_mut().write_message(BossDefeated(BossId::BossB));
        app.update();

        assert!(progress(&app).dual_boss_unlocked);
    }

    #[test]
    fn player_death_shows_a_defeat_result() {
        let mut app = setup_app();
        set_phase(&mut app, DayPhase::BossFight);

        app.world_mut().write_message(PlayerDied);
        app.update();
        app.update();

        assert_eq!(day_cycle(&app).outcome, Some(Outcome::Defeat));
        assert_eq!(
            app.world().resource::<State<DayPhase>>().get(),
            &DayPhase::Result
        );
    }

    #[test]
    fn leaving_the_result_advances_to_the_farm() {
        let mut app = setup_app();
        set_phase(&mut app, DayPhase::Result);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<State<DayPhase>>().get(),
            &DayPhase::Farming
        );
        assert_eq!(
            app.world().resource::<LevelRequest>().0,
            Some(LevelId::Farm)
        );
        assert!(day_cycle(&app).pending_advance);
    }

    #[test]
    fn a_dual_victory_ends_the_run() {
        let mut app = setup_app();
        set_phase(&mut app, DayPhase::BossFight);
        app.world_mut().write_message(BossDefeated(BossId::Dual));
        app.update();
        assert!(day_cycle(&app).run_complete);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<State<GameState>>().get(),
            &GameState::Victory
        );
    }

    #[test]
    fn the_result_screen_spawns_and_despawns() {
        let mut app = setup_app();
        set_phase(&mut app, DayPhase::Result);
        assert!(
            app.world_mut()
                .query_filtered::<Entity, With<ResultScreenRoot>>()
                .iter(app.world())
                .next()
                .is_some()
        );

        set_phase(&mut app, DayPhase::Farming);
        assert!(
            app.world_mut()
                .query_filtered::<Entity, With<ResultScreenRoot>>()
                .iter(app.world())
                .next()
                .is_none(),
            "result screen is cleaned up on the way out"
        );
    }
}
