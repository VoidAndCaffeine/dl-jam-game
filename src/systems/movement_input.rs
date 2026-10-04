use crate::components::player::Movement;
use crate::resources::crafting_menu::CraftingMenu;
use crate::resources::inventory_panel::InventoryPanel;
use bevy::prelude::*;

pub fn movement_input(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut movement_query: Query<&mut Movement>,
    menu: Res<CraftingMenu>,
    inventory: Res<InventoryPanel>,
) {
    let frozen = menu.open || inventory.open;
    for mut movement in movement_query.iter_mut() {
        let mut direction = Vec2::ZERO;

        if keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp) {
            direction.y += 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown) {
            direction.y -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft) {
            direction.x -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight) {
            direction.x += 1.0;
        }

        movement.input_direction = if frozen {
            Vec2::ZERO
        } else {
            direction.normalize_or_zero()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<CraftingMenu>()
            .init_resource::<InventoryPanel>();
        app.world_mut().spawn(Movement::default());
        app.add_systems(Update, movement_input);
        app
    }

    fn get_movement(app: &mut App) -> Movement {
        app.world_mut()
            .query::<&Movement>()
            .single(app.world_mut())
            .unwrap()
            .clone()
    }

    #[test]
    fn no_input_returns_zero_direction() {
        let mut app = setup_app();
        app.update();
        let movement = get_movement(&mut app);
        assert_eq!(movement.input_direction, Vec2::ZERO);
    }

    #[test]
    fn wasd_sets_direction() {
        let mut app = setup_app();
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::KeyW);
        input.press(KeyCode::KeyD);
        app.update();
        let movement = get_movement(&mut app);
        assert_eq!(movement.input_direction, Vec2::new(1.0, 1.0).normalize());
    }

    #[test]
    fn arrow_keys_set_direction() {
        let mut app = setup_app();
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::ArrowUp);
        input.press(KeyCode::ArrowLeft);
        app.update();
        let movement = get_movement(&mut app);
        assert_eq!(movement.input_direction, Vec2::new(-1.0, 1.0).normalize());
    }

    #[test]
    fn opposite_keys_cancel_out() {
        let mut app = setup_app();
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::KeyW);
        input.press(KeyCode::KeyS);
        app.update();
        let movement = get_movement(&mut app);
        assert_eq!(movement.input_direction, Vec2::ZERO);
    }

    #[test]
    fn diagonal_normalized_to_unit_length() {
        let mut app = setup_app();
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::KeyW);
        input.press(KeyCode::KeyD);
        app.update();
        let movement = get_movement(&mut app);
        assert!((movement.input_direction.length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn movement_is_frozen_while_the_crafting_menu_is_open() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<CraftingMenu>().open = true;
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::KeyD);
        app.update();
        assert_eq!(get_movement(&mut app).input_direction, Vec2::ZERO);
    }

    #[test]
    fn movement_resumes_after_the_menu_closes() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<CraftingMenu>().open = true;
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::KeyD);
        app.update();

        app.world_mut().resource_mut::<CraftingMenu>().open = false;
        app.update();
        assert_eq!(get_movement(&mut app).input_direction, Vec2::X);
    }

    #[test]
    fn movement_is_frozen_while_the_inventory_is_open() {
        let mut app = setup_app();
        app.world_mut()
            .resource_mut::<InventoryPanel>()
            .open_panel();
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::KeyD);
        app.update();
        assert_eq!(get_movement(&mut app).input_direction, Vec2::ZERO);
    }

    #[test]
    fn movement_resumes_after_the_inventory_closes() {
        let mut app = setup_app();
        app.world_mut()
            .resource_mut::<InventoryPanel>()
            .open_panel();
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::KeyD);
        app.update();

        app.world_mut()
            .resource_mut::<InventoryPanel>()
            .close_panel();
        app.update();
        assert_eq!(get_movement(&mut app).input_direction, Vec2::X);
    }
}
