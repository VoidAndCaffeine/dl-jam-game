use crate::components::boss::BossId;
use crate::events::{BossSelected, InteractionEvent, InteractionType, PlaySfx, Sfx};
use crate::resources::boss_progress::BossProgress;
use crate::resources::boss_select::BossSelectMenu;
use crate::resources::infinite_mode::InfiniteMode;
use crate::resources::level::LevelRequest;
use crate::resources::scene_assets::{LoadTarget, LoadingContext};
use crate::states::{DayPhase, Phase};
use crate::systems::scene_fade::SceneFade;
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::prelude::*;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum BossSelectSet {
    Menu,
}

/// A clickable boss row in the selection panel.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct BossOption {
    pub boss_id: BossId,
    /// Index into [`BossId::ALL`].
    pub index: usize,
}

/// Opens the selection panel the moment the farm's arena gate is used. No fade
/// here: the menu is shown first, and only choosing a boss fades out to the
/// arena.
fn open_boss_select_on_gate(
    mut events: MessageReader<InteractionEvent>,
    mut menu: ResMut<BossSelectMenu>,
    mut next_phase: ResMut<NextState<DayPhase>>,
    phase: Phase,
) {
    // Drain every event even outside farming so stale clicks cannot reopen the
    // menu once the farm is back.
    for event in events.read() {
        if !phase.is_farming() {
            continue;
        }
        if event.interaction_type == InteractionType::BossArena {
            menu.open_menu();
            next_phase.set(DayPhase::BossSelect);
        }
    }
}

/// Moves the highlight, skipping options that are still locked.
fn step_selection(current: usize, direction: i32, progress: &BossProgress) -> usize {
    let last = BossId::ALL.len() as i32 - 1;
    let mut index = current as i32 + direction;
    while (0..=last).contains(&index) {
        if progress.is_unlocked(BossId::ALL[index as usize]) {
            return index as usize;
        }
        index += direction;
    }
    current
}

/// Keyboard handling for both the list and the confirmation dialog.
#[allow(clippy::too_many_arguments)]
fn boss_select_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<BossSelectMenu>,
    progress: Res<BossProgress>,
    mut infinite: ResMut<InfiniteMode>,
    mut context: ResMut<LoadingContext>,
    mut fade: ResMut<SceneFade>,
    mut next_phase: ResMut<NextState<DayPhase>>,
    mut request: ResMut<LevelRequest>,
    mut selected_events: MessageWriter<BossSelected>,
    mut sfx: MessageWriter<PlaySfx>,
    phase: Phase,
) {
    if !phase.is_boss_select() || !menu.open {
        return;
    }

    if menu.confirmation_open {
        if crate::plugins::ui_theme::back_pressed(&keys) || keys.just_pressed(KeyCode::KeyN) {
            menu.close_confirmation();
        } else if (keys.just_pressed(KeyCode::Enter)
            || keys.just_pressed(KeyCode::Space)
            || keys.just_pressed(KeyCode::KeyY))
            && let Some(id) = menu.pending_boss
        {
            selected_events.write(BossSelected(id));
            sfx.write(PlaySfx(Sfx::MenuConfirm));
            // Infinite Mode reuses the dual arena but fields two independent
            // bosses; every other choice clears it.
            if id == BossId::Infinite {
                infinite.begin();
            } else {
                infinite.end();
            }
            request.0 = Some(id.arena());
            fade.begin_load(LoadTarget::for_boss(id), DayPhase::BossFight, &mut context);
            menu.close_menu();
        }
        return;
    }

    if crate::plugins::ui_theme::back_pressed(&keys) {
        menu.close_menu();
        next_phase.set(DayPhase::Farming);
        return;
    }

    if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) {
        menu.selected = step_selection(menu.selected, -1, &progress);
    } else if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) {
        menu.selected = step_selection(menu.selected, 1, &progress);
    }

    let opens = keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::KeyE);
    if opens
        && let Some(id) = BossId::ALL.get(menu.selected).copied()
        && progress.is_unlocked(id)
    {
        menu.open_confirmation(id);
    }
}

/// Clicking an unlocked row selects it and asks for confirmation.
fn select_boss_on_click(
    options: Query<(Entity, &BossOption), Changed<Interaction>>,
    interactions: Query<&Interaction>,
    mut menu: ResMut<BossSelectMenu>,
    progress: Res<BossProgress>,
    phase: Phase,
) {
    if !phase.is_boss_select() || !menu.open || menu.confirmation_open {
        return;
    }
    for (entity, option) in options.iter() {
        if interactions.get(entity) != Ok(&Interaction::Pressed) {
            continue;
        }
        if !progress.is_unlocked(option.boss_id) {
            continue;
        }
        menu.selected = option.index;
        menu.open_confirmation(option.boss_id);
    }
}

/// Mirrors the boss row under the mouse into `menu.hovered` so the concept pane
/// follows the cursor. The keyboard `selected` row keeps the highlight and
/// hover never opens a confirmation.
fn track_boss_hover(options: Query<(&BossOption, &Interaction)>, mut menu: ResMut<BossSelectMenu>) {
    if !menu.open {
        return;
    }
    let hovered = crate::systems::crafting::hovered_row(
        options
            .iter()
            .map(|(option, interaction)| (option.index, interaction)),
    );
    if menu.hovered != hovered {
        menu.hovered = hovered;
    }
}

/// The menu is the thing that makes the `BossSelect` phase meaningful. If some
/// other panel closes it, fall back to farming instead of getting stuck.
fn recover_closed_boss_select(
    menu: Res<BossSelectMenu>,
    fade: Res<SceneFade>,
    mut next_phase: ResMut<NextState<DayPhase>>,
    phase: Phase,
) {
    // A fade in progress means the menu closed on purpose (a boss was confirmed
    // or the player backed out); leave the transition to the fade.
    if !fade.is_idle() {
        return;
    }
    if phase.is_boss_select() && !menu.open {
        next_phase.set(DayPhase::Farming);
    }
}

pub struct BossSelectPlugin;

impl Plugin for BossSelectPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BossSelectMenu>()
            .add_message::<BossSelected>()
            .add_message::<crate::events::PlaySfx>()
            .add_systems(Update, open_boss_select_on_gate.in_set(BossSelectSet::Menu))
            .add_systems(
                Update,
                (
                    recover_closed_boss_select,
                    boss_select_keyboard,
                    select_boss_on_click,
                    track_boss_hover,
                )
                    .chain()
                    .in_set(BossSelectSet::Menu),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::interaction::BossArenaEntry;
    use crate::plugins::loading::LoadingPlugin;
    use crate::resources::boss_progress::BossProgress;
    use crate::resources::boss_sprite::BossSpriteAssets;
    use crate::resources::level::LevelRequest;
    use crate::resources::player_sprite::PlayerSpriteAssets;
    use crate::resources::run_data::PlayerGear;
    use crate::resources::scene_assets::LoadingContext;
    use crate::states::GameState;
    use bevy::state::app::StatesPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<BossSelectMenu>()
            .init_resource::<BossProgress>()
            .init_resource::<InfiniteMode>()
            .init_resource::<LevelRequest>()
            .init_resource::<PlayerGear>()
            .init_resource::<PlayerSpriteAssets>()
            .init_resource::<BossSpriteAssets>()
            .add_plugins((
                MinimalPlugins,
                StatesPlugin,
                BossSelectPlugin,
                LoadingPlugin,
                crate::systems::scene_fade::SceneFadePlugin,
            ))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_message::<InteractionEvent>();

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);
        app.update();
        app
    }

    fn menu(app: &App) -> &BossSelectMenu {
        app.world().resource::<BossSelectMenu>()
    }

    fn phase(app: &App) -> DayPhase {
        app.world().resource::<State<DayPhase>>().get().clone()
    }

    fn request(app: &App) -> Option<crate::levels::LevelId> {
        app.world().resource::<LevelRequest>().0
    }

    fn context(app: &App) -> &LoadingContext {
        app.world().resource::<LoadingContext>()
    }

    /// Runs enough frames for input to latch and any loading step to finish.
    fn settle(app: &mut App) {
        for _ in 0..4 {
            app.update();
        }
    }

    fn press(app: &mut App, key: KeyCode) {
        {
            let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            input.release(key);
            input.press(key);
        }
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear_just_pressed(key);
        settle(app);
    }

    fn set_phase(app: &mut App, day: DayPhase) {
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(day);
        app.update();
    }

    fn arena_gate_interaction(app: &mut App) {
        let entity = app.world_mut().spawn(BossArenaEntry).id();
        app.world_mut().write_message(InteractionEvent {
            entity,
            interaction_type: InteractionType::BossArena,
        });
        settle(app);
    }

    #[test]
    fn the_gate_opens_the_menu_and_moves_to_boss_select() {
        let mut app = setup_app();
        assert!(!menu(&app).open);

        arena_gate_interaction(&mut app);

        assert!(menu(&app).open);
        assert_eq!(phase(&app), DayPhase::BossSelect);
        assert_eq!(
            context(&app).target,
            None,
            "opening the menu does not load a scene"
        );
    }

    #[test]
    fn escape_closes_the_menu_and_returns_to_farming() {
        let mut app = setup_app();
        arena_gate_interaction(&mut app);
        assert!(menu(&app).open);

        press(&mut app, KeyCode::Escape);

        assert!(!menu(&app).open);
        assert_eq!(phase(&app), DayPhase::Farming);
        assert_eq!(request(&app), None, "no arena is loaded when cancelling");
    }

    #[test]
    fn selecting_boss_a_opens_a_confirmation_then_loads_its_arena() {
        let mut app = setup_app();
        arena_gate_interaction(&mut app);

        press(&mut app, KeyCode::Enter);
        assert!(menu(&app).confirmation_open);
        assert_eq!(menu(&app).pending_boss, Some(BossId::BossA));
        assert_eq!(phase(&app), DayPhase::BossSelect, "not fighting yet");

        press(&mut app, KeyCode::Enter);
        assert_eq!(phase(&app), DayPhase::BossFight);
        assert_eq!(request(&app), Some(crate::levels::LevelId::ArenaA));
        assert_eq!(context(&app).target, Some(LoadTarget::ArenaA));
        assert!(!menu(&app).open);
    }

    #[test]
    fn cancelling_the_confirmation_keeps_the_menu_open() {
        let mut app = setup_app();
        arena_gate_interaction(&mut app);
        press(&mut app, KeyCode::Enter);

        press(&mut app, KeyCode::Escape);

        assert!(menu(&app).open);
        assert!(!menu(&app).confirmation_open);
        assert_eq!(phase(&app), DayPhase::BossSelect);
    }

    #[test]
    fn navigation_skips_the_locked_dual_boss() {
        let mut app = setup_app();
        arena_gate_interaction(&mut app);

        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(menu(&app).selected, 1);
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(
            menu(&app).selected,
            1,
            "dual is locked until both bosses fall"
        );
    }

    #[test]
    fn navigation_reaches_dual_once_both_bosses_are_beaten() {
        let mut app = setup_app();
        {
            let mut progress = app.world_mut().resource_mut::<BossProgress>();
            progress.record(BossId::BossA);
            progress.record(BossId::BossB);
        }
        arena_gate_interaction(&mut app);

        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(menu(&app).selected, 2);
    }

    #[test]
    fn launching_boss_b_requests_arena_b() {
        let mut app = setup_app();
        arena_gate_interaction(&mut app);
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);

        assert_eq!(request(&app), Some(crate::levels::LevelId::ArenaB));
    }

    #[test]
    fn selecting_infinite_mode_starts_the_endurance_fight() {
        let mut app = setup_app();
        {
            let mut progress = app.world_mut().resource_mut::<BossProgress>();
            progress.record(BossId::BossA);
            progress.record(BossId::BossB);
            progress.record(BossId::Dual);
        }
        arena_gate_interaction(&mut app);

        // 0 -> 1 -> 2 -> 3, all unlocked once the dual boss falls.
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::ArrowDown);
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(menu(&app).selected, 3);

        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);

        assert!(
            app.world().resource::<InfiniteMode>().active,
            "Infinite Mode is armed"
        );
        assert_eq!(request(&app), Some(crate::levels::LevelId::ArenaDual));
    }

    #[test]
    fn picking_a_normal_boss_clears_infinite_mode() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<InfiniteMode>().begin();
        arena_gate_interaction(&mut app);

        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);

        assert!(
            !app.world().resource::<InfiniteMode>().active,
            "a normal fight never runs in endurance mode"
        );
    }

    #[test]
    fn clicking_a_locked_option_does_nothing() {
        let mut app = setup_app();
        arena_gate_interaction(&mut app);

        let option = app
            .world_mut()
            .spawn((
                BossOption {
                    boss_id: BossId::Dual,
                    index: 2,
                },
                Interaction::None,
            ))
            .id();
        app.update();
        *app.world_mut().get_mut::<Interaction>(option).unwrap() = Interaction::Pressed;
        app.update();

        assert!(!menu(&app).confirmation_open);
    }

    #[test]
    fn clicking_boss_b_opens_its_confirmation() {
        let mut app = setup_app();
        arena_gate_interaction(&mut app);

        let option = app
            .world_mut()
            .spawn((
                BossOption {
                    boss_id: BossId::BossB,
                    index: 1,
                },
                Interaction::None,
            ))
            .id();
        app.update();
        *app.world_mut().get_mut::<Interaction>(option).unwrap() = Interaction::Pressed;
        app.update();

        assert!(menu(&app).confirmation_open);
        assert_eq!(menu(&app).pending_boss, Some(BossId::BossB));
    }

    #[test]
    fn hovering_a_boss_row_previews_it_without_confirming() {
        let mut app = setup_app();
        arena_gate_interaction(&mut app);

        app.world_mut().spawn((
            BossOption {
                boss_id: BossId::BossB,
                index: 1,
            },
            Interaction::Hovered,
        ));
        app.update();

        assert_eq!(menu(&app).hovered, Some(1));
        assert_eq!(menu(&app).selected, 0, "the highlight does not move");
        assert!(!menu(&app).confirmation_open, "hover never confirms");
    }

    #[test]
    fn leaving_a_boss_row_clears_the_preview() {
        let mut app = setup_app();
        arena_gate_interaction(&mut app);

        let option = app
            .world_mut()
            .spawn((
                BossOption {
                    boss_id: BossId::BossB,
                    index: 1,
                },
                Interaction::Hovered,
            ))
            .id();
        app.update();
        assert_eq!(menu(&app).hovered, Some(1));

        *app.world_mut().get_mut::<Interaction>(option).unwrap() = Interaction::None;
        app.update();
        assert_eq!(menu(&app).hovered, None);
    }

    #[test]
    fn closing_the_menu_from_another_panel_recovers_to_farming() {
        let mut app = setup_app();
        set_phase(&mut app, DayPhase::BossSelect);
        assert!(!menu(&app).open);

        app.update();

        assert_eq!(phase(&app), DayPhase::Farming);
    }
}
