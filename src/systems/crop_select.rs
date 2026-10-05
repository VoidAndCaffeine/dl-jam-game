use crate::components::pot::{CropType, Pot, PotState};
use crate::events::CropPlanted;
use crate::resources::crop_select::CropSelectMenu;
use crate::resources::farm::CropUnlocks;
use crate::states::Phase;
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CropSelectSet {
    Menu,
}

/// A clickable crop row in the selection panel.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct CropOption {
    pub crop: CropType,
    /// Index into [`CropType::ALL`].
    pub index: usize,
}

/// Moves the highlight, skipping crops that are still locked.
fn step_selection(current: usize, direction: i32, unlocks: &CropUnlocks) -> usize {
    let last = CropType::ALL.len() as i32 - 1;
    let mut index = current as i32 + direction;
    while (0..=last).contains(&index) {
        if unlocks.is_unlocked(CropType::ALL[index as usize]) {
            return index as usize;
        }
        index += direction;
    }
    current
}

/// Plants the highlighted crop into the pending pot and closes the picker. If the
/// crop is locked, the pot is gone, or the pot is no longer empty, nothing is
/// planted.
fn plant_selected(
    menu: &mut CropSelectMenu,
    unlocks: &CropUnlocks,
    pots: &mut Query<&mut Pot>,
    planted_events: &mut MessageWriter<CropPlanted>,
) {
    let Some(crop) = CropType::ALL.get(menu.selected).copied() else {
        menu.close_menu();
        return;
    };
    if !unlocks.is_unlocked(crop) {
        return;
    }
    let pending = menu.pending_pot;
    let Some(mut pot) = pots.iter_mut().find(|pot| pot.index == pending) else {
        menu.close_menu();
        return;
    };
    if pot.state != PotState::Empty {
        menu.close_menu();
        return;
    }
    pot.plant(crop);
    planted_events.write(CropPlanted(crop));
    menu.close_menu();
}

/// The resources shared by both planting entry points.
#[derive(bevy::ecs::system::SystemParam)]
pub struct PlantingWork<'w, 's> {
    pub menu: ResMut<'w, CropSelectMenu>,
    pub unlocks: Res<'w, CropUnlocks>,
    pub pots: Query<'w, 's, &'static mut Pot>,
    pub planted_events: MessageWriter<'w, CropPlanted>,
}

impl PlantingWork<'_, '_> {
    fn plant(&mut self) {
        plant_selected(
            &mut self.menu,
            &self.unlocks,
            &mut self.pots,
            &mut self.planted_events,
        );
    }
}

/// Keyboard handling: navigate the crop list and confirm a choice.
fn crop_select_keyboard(keys: Res<ButtonInput<KeyCode>>, mut work: PlantingWork, phase: Phase) {
    if !phase.is_farming() || !work.menu.open {
        return;
    }

    if keys.just_pressed(KeyCode::Escape) {
        work.menu.close_menu();
        return;
    }

    if keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW) {
        work.menu.selected = step_selection(work.menu.selected, -1, &work.unlocks);
    } else if keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS) {
        work.menu.selected = step_selection(work.menu.selected, 1, &work.unlocks);
    }

    let confirms = keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::KeyE);
    if confirms {
        work.plant();
    }
}

/// Clicking an unlocked row plants that crop immediately. The fresh-click guard
/// stops the click that opened the picker (if the mouse is still held) from
/// planting a row that spawns under the cursor.
fn select_crop_on_click(
    rows: Query<(Entity, &CropOption), Changed<Interaction>>,
    interactions: Query<&Interaction>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut work: PlantingWork,
    phase: Phase,
) {
    if !phase.is_farming() || !work.menu.open {
        return;
    }
    if !mouse_input.just_pressed(MouseButton::Left) {
        return;
    }
    for (entity, option) in rows.iter() {
        if interactions.get(entity) != Ok(&Interaction::Pressed) {
            continue;
        }
        if !work.unlocks.is_unlocked(option.crop) {
            continue;
        }
        work.menu.selected = option.index;
        work.plant();
        return;
    }
}

/// The picker only belongs in the farming phase; if something moves the day on
/// while it is open, drop it rather than leaving a stale panel behind.
fn close_crop_select_outside_farming(mut menu: ResMut<CropSelectMenu>, phase: Phase) {
    if menu.open && !phase.is_farming() {
        menu.close_menu();
    }
}

pub struct CropSelectPlugin;

impl Plugin for CropSelectPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CropSelectMenu>()
            .add_message::<CropPlanted>()
            .add_systems(
                Update,
                (
                    close_crop_select_outside_farming,
                    crop_select_keyboard,
                    select_crop_on_click,
                )
                    .chain()
                    .in_set(CropSelectSet::Menu),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::states::{DayPhase, GameState};
    use bevy::ecs::message::MessageReader;
    use bevy::state::app::StatesPlugin;

    #[derive(Resource, Default)]
    struct CapturedPlanted(Vec<CropType>);

    fn capture_planted(
        mut captured: ResMut<CapturedPlanted>,
        mut reader: MessageReader<CropPlanted>,
    ) {
        for event in reader.read() {
            captured.0.push(event.0);
        }
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<CropUnlocks>()
            .init_resource::<CapturedPlanted>()
            .add_plugins((MinimalPlugins, StatesPlugin, CropSelectPlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_systems(Update, capture_planted.after(CropSelectSet::Menu));

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);
        app.update();
        app
    }

    fn spawn_pot(app: &mut App, index: usize) -> Entity {
        app.world_mut().spawn(Pot::new(index)).id()
    }

    fn open(app: &mut App, pot_index: usize) {
        app.world_mut()
            .resource_mut::<CropSelectMenu>()
            .open_menu(pot_index);
    }

    fn menu(app: &App) -> &CropSelectMenu {
        app.world().resource::<CropSelectMenu>()
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
        app.update();
    }

    fn click(app: &mut App, mouse: MouseButton) {
        {
            let mut input = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            input.release(mouse);
            input.press(mouse);
        }
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(mouse);
        app.update();
    }

    fn planted(app: &App) -> Vec<CropType> {
        app.world().resource::<CapturedPlanted>().0.clone()
    }

    fn unlock_a(app: &mut App) {
        app.world_mut()
            .resource_mut::<CropUnlocks>()
            .unlock_crop_a();
    }

    fn unlock_b(app: &mut App) {
        app.world_mut()
            .resource_mut::<CropUnlocks>()
            .unlock_crop_b();
    }

    fn pot(app: &App, entity: Entity) -> &Pot {
        app.world().get::<Pot>(entity).expect("pot exists")
    }

    #[test]
    fn step_selection_skips_locked_crops() {
        let unlocks = CropUnlocks::new();
        assert_eq!(
            step_selection(0, 1, &unlocks),
            0,
            "only starter is unlocked"
        );
    }

    #[test]
    fn step_selection_reaches_unlocked_crops_in_order() {
        let mut unlocks = CropUnlocks::new();
        unlocks.unlock_crop_a();
        unlocks.unlock_crop_b();
        assert_eq!(step_selection(0, 1, &unlocks), 1);
        assert_eq!(step_selection(1, 1, &unlocks), 2);
        assert_eq!(step_selection(1, -1, &unlocks), 0);
    }

    #[test]
    fn step_selection_stops_at_the_edges() {
        let unlocks = CropUnlocks::new();
        assert_eq!(step_selection(0, -1, &unlocks), 0);
    }

    #[test]
    fn enter_plants_the_selected_crop() {
        let mut app = setup_app();
        let pot_entity = spawn_pot(&mut app, 0);
        open(&mut app, 0);

        press(&mut app, KeyCode::Enter);

        assert_eq!(pot(&app, pot_entity).state, PotState::Watered);
        assert_eq!(pot(&app, pot_entity).crop_type, CropType::Starter);
        assert_eq!(planted(&app), vec![CropType::Starter]);
        assert!(!menu(&app).open, "planting closes the picker");
    }

    #[test]
    fn e_and_space_also_plant() {
        let mut app = setup_app();
        spawn_pot(&mut app, 0);
        spawn_pot(&mut app, 1);
        unlock_a(&mut app);

        open(&mut app, 0);
        app.world_mut().resource_mut::<CropSelectMenu>().selected = 1;
        press(&mut app, KeyCode::KeyE);

        open(&mut app, 1);
        app.world_mut().resource_mut::<CropSelectMenu>().selected = 1;
        press(&mut app, KeyCode::Space);

        assert_eq!(planted(&app), vec![CropType::CropA, CropType::CropA]);
    }

    #[test]
    fn escape_closes_without_planting() {
        let mut app = setup_app();
        let pot_entity = spawn_pot(&mut app, 0);
        open(&mut app, 0);

        press(&mut app, KeyCode::Escape);

        assert!(!menu(&app).open);
        assert_eq!(pot(&app, pot_entity).state, PotState::Empty);
        assert!(planted(&app).is_empty());
    }

    #[test]
    fn navigation_skips_locked_crops() {
        let mut app = setup_app();
        spawn_pot(&mut app, 0);
        open(&mut app, 0);

        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(menu(&app).selected, 0, "only starter is unlocked");

        unlock_a(&mut app);
        press(&mut app, KeyCode::ArrowDown);
        assert_eq!(menu(&app).selected, 1);
    }

    #[test]
    fn click_plants_an_unlocked_row() {
        let mut app = setup_app();
        let pot_entity = spawn_pot(&mut app, 0);
        unlock_b(&mut app);
        open(&mut app, 0);

        let row = app
            .world_mut()
            .spawn((
                CropOption {
                    crop: CropType::CropB,
                    index: 2,
                },
                Interaction::None,
            ))
            .id();
        app.update();
        *app.world_mut().get_mut::<Interaction>(row).unwrap() = Interaction::Pressed;
        click(&mut app, MouseButton::Left);

        assert_eq!(pot(&app, pot_entity).crop_type, CropType::CropB);
        assert_eq!(planted(&app), vec![CropType::CropB]);
        assert!(!menu(&app).open);
    }

    #[test]
    fn click_on_a_locked_row_does_nothing() {
        let mut app = setup_app();
        let pot_entity = spawn_pot(&mut app, 0);
        open(&mut app, 0);

        let row = app
            .world_mut()
            .spawn((
                CropOption {
                    crop: CropType::CropB,
                    index: 2,
                },
                Interaction::None,
            ))
            .id();
        app.update();
        *app.world_mut().get_mut::<Interaction>(row).unwrap() = Interaction::Pressed;
        click(&mut app, MouseButton::Left);

        assert_eq!(pot(&app, pot_entity).state, PotState::Empty);
        assert!(planted(&app).is_empty());
        assert!(menu(&app).open);
    }

    #[test]
    fn a_held_click_does_not_plant_when_the_picker_opens() {
        let mut app = setup_app();
        spawn_pot(&mut app, 0);

        // The click that opens the picker is a fresh press, but the picker's rows
        // do not exist yet, so nothing can be under the cursor.
        {
            let mut input = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
            input.press(MouseButton::Left);
        }
        open(&mut app, 0);
        app.update();

        // End of frame: the button is still held but no longer freshly pressed.
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear_just_pressed(MouseButton::Left);

        // Next frame the row exists under the held cursor and reports Pressed.
        let row = app
            .world_mut()
            .spawn((
                CropOption {
                    crop: CropType::Starter,
                    index: 0,
                },
                Interaction::None,
            ))
            .id();
        app.update();
        *app.world_mut().get_mut::<Interaction>(row).unwrap() = Interaction::Pressed;
        app.update();

        assert!(
            planted(&app).is_empty(),
            "holding the button must not plant on open"
        );
        assert!(menu(&app).open);
    }

    #[test]
    fn planting_into_a_missing_pot_closes_the_picker() {
        let mut app = setup_app();
        open(&mut app, 5);

        press(&mut app, KeyCode::Enter);

        assert!(!menu(&app).open);
        assert!(planted(&app).is_empty());
    }

    #[test]
    fn planting_into_a_non_empty_pot_does_nothing() {
        let mut app = setup_app();
        let pot_entity = spawn_pot(&mut app, 0);
        {
            let mut pot = app.world_mut().get_mut::<Pot>(pot_entity).unwrap();
            pot.plant(CropType::Starter);
            pot.state = PotState::Watered;
        }
        open(&mut app, 0);

        press(&mut app, KeyCode::Enter);

        assert!(planted(&app).is_empty());
        assert_eq!(pot(&app, pot_entity).crop_type, CropType::Starter);
        assert!(!menu(&app).open);
    }

    #[test]
    fn keyboard_does_nothing_while_the_picker_is_closed() {
        let mut app = setup_app();
        let pot_entity = spawn_pot(&mut app, 0);

        press(&mut app, KeyCode::Enter);

        assert_eq!(pot(&app, pot_entity).state, PotState::Empty);
    }

    #[test]
    fn keyboard_does_nothing_outside_farming() {
        let mut app = setup_app();
        let pot_entity = spawn_pot(&mut app, 0);
        open(&mut app, 0);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossSelect);
        app.update();

        press(&mut app, KeyCode::Enter);

        assert_eq!(pot(&app, pot_entity).state, PotState::Empty);
        assert!(!menu(&app).open, "leaving farming closes the picker");
    }

    #[test]
    fn leaving_farming_closes_the_picker_even_without_input() {
        let mut app = setup_app();
        open(&mut app, 0);

        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossFight);
        app.update();

        assert!(!menu(&app).open);
    }
}
