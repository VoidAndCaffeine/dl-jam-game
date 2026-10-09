use crate::components::player::{INTERACTION_RANGE, Player};
use crate::events::InteractionEvent;
use crate::materials::sprite_outline::SpriteOutlineMaterial;
use crate::resources::crafting_menu::CraftingMenu;
use crate::resources::crop_select::CropSelectMenu;
use crate::resources::inventory_panel::InventoryPanel;
use crate::states::Phase;
use crate::utils::interaction_math::{
    find_closest_in_range, find_closest_to_ray, resolve_interaction_type_from_queries,
};
use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::sprite_render::MeshMaterial2d;

#[derive(Component, Reflect, Default, Debug)]
pub struct Interactable;

impl Interactable {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Component, Reflect, Default, Debug)]
pub struct HighlightMarker;

/// Highlights sit just behind their parent sprite. The room floor must stay
/// further back than this or it covers them.
pub const HIGHLIGHT_Z: f32 = -0.1;

#[derive(Component, Reflect, Default, Debug)]
pub struct FarmPot;

#[derive(Component, Reflect, Default, Debug)]
pub struct CraftingStation;

#[derive(Component, Reflect, Default, Debug)]
pub struct BossArenaEntry;

#[derive(Component, Reflect, Default, Debug)]
pub struct NPC;

/// Everything both interaction systems need to find and classify interactables.
#[derive(bevy::ecs::system::SystemParam)]
pub struct InteractableLookups<'w, 's> {
    pub player: Query<'w, 's, &'static GlobalTransform, With<Player>>,
    pub interactables: Query<'w, 's, (Entity, &'static GlobalTransform), With<Interactable>>,
    pub farm_pots: Query<'w, 's, &'static FarmPot>,
    pub pots: Query<'w, 's, &'static crate::components::pot::Pot>,
    pub crafting_stations: Query<'w, 's, &'static CraftingStation>,
    pub boss_arenas: Query<'w, 's, &'static BossArenaEntry>,
    pub npcs: Query<'w, 's, &'static NPC>,
}

/// The panels that freeze world interaction while they are open.
#[derive(bevy::ecs::system::SystemParam)]
pub struct OpenPanels<'w> {
    menu: Res<'w, CraftingMenu>,
    inventory: Res<'w, InventoryPanel>,
    crop_select: Res<'w, CropSelectMenu>,
    pause: Option<Res<'w, crate::resources::pause::PauseMenu>>,
}

impl OpenPanels<'_> {
    pub fn any_open(&self) -> bool {
        self.menu.open
            || self.inventory.open
            || self.crop_select.open
            || self.pause.as_ref().is_some_and(|pause| pause.open)
    }

    /// True when a panel other than the pause overlay is open. Used so `Esc`
    /// opens the pause menu only when nothing else has focus.
    pub fn any_open_except_pause(&self) -> bool {
        self.menu.open || self.inventory.open || self.crop_select.open
    }
}

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InventoryPanel>()
            .init_resource::<CropSelectMenu>()
            .add_message::<InteractionEvent>()
            .add_systems(Update, player_proximity_interaction)
            .add_systems(Update, mouse_raycast_interaction)
            .add_systems(Update, highlight_interactables_in_range);
    }
}

fn player_proximity_interaction(
    mut events: MessageWriter<InteractionEvent>,
    lookups: InteractableLookups,
    keys: Res<ButtonInput<KeyCode>>,
    panels: OpenPanels,
    phase: Phase,
) {
    if !phase.is_farming() || panels.any_open() {
        return;
    }
    if !keys.just_pressed(KeyCode::Space) {
        return;
    }

    let Ok(player_transform) = lookups.player.single() else {
        return;
    };
    let player_pos = player_transform.translation().truncate();

    let interactable_positions: Vec<(Entity, Vec2)> = lookups
        .interactables
        .iter()
        .filter(|(entity, _)| {
            // Skip watered pots - they are not interactable until next day
            lookups
                .pots
                .get(*entity)
                .map(|p| p.state != crate::components::pot::PotState::Watered)
                .unwrap_or(true)
        })
        .map(|(entity, transform)| (entity, transform.translation().truncate()))
        .collect();

    log::debug!(
        "space pressed at {player_pos:?} with {} interactables",
        interactable_positions.len()
    );

    let closest_entity =
        find_closest_in_range(player_pos, &interactable_positions, INTERACTION_RANGE);

    if let Some(entity) = closest_entity {
        let interaction_type = resolve_interaction_type_from_queries(
            entity,
            &lookups.farm_pots,
            &lookups.crafting_stations,
            &lookups.boss_arenas,
            &lookups.npcs,
        );
        events.write(InteractionEvent {
            entity,
            interaction_type,
        });
    }
}

fn mouse_raycast_interaction(
    mut events: MessageWriter<InteractionEvent>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    lookups: InteractableLookups,
    mouse_input: Res<ButtonInput<MouseButton>>,
    panels: OpenPanels,
    phase: Phase,
) {
    if !phase.is_farming() || panels.any_open() {
        return;
    }
    if !mouse_input.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = windows.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) else {
        return;
    };

    let Ok(player_transform) = lookups.player.single() else {
        return;
    };
    let player_pos = player_transform.translation().truncate();

    let interactable_positions: Vec<(Entity, Vec2)> = lookups
        .interactables
        .iter()
        .filter(|(entity, _)| {
            // Skip watered pots - they are not interactable until next day
            lookups
                .pots
                .get(*entity)
                .map(|p| p.state != crate::components::pot::PotState::Watered)
                .unwrap_or(true)
        })
        .map(|(entity, transform)| (entity, transform.translation().truncate()))
        .collect();

    log::debug!(
        "left click at {cursor_pos:?}, ray from {:?}, {} interactables",
        ray.origin.truncate(),
        interactable_positions.len()
    );

    let in_range: Vec<(Entity, Vec2)> = interactable_positions
        .into_iter()
        .filter(|(_, pos)| player_pos.distance(*pos) <= INTERACTION_RANGE)
        .collect();

    let closest_entity = find_closest_to_ray(ray.origin.truncate(), &in_range);

    if let Some(entity) = closest_entity {
        let interaction_type = resolve_interaction_type_from_queries(
            entity,
            &lookups.farm_pots,
            &lookups.crafting_stations,
            &lookups.boss_arenas,
            &lookups.npcs,
        );
        events.write(InteractionEvent {
            entity,
            interaction_type,
        });
    }
}

/// Everything the highlight system reads to decide what to light up.
#[derive(bevy::ecs::system::SystemParam)]
pub struct HighlightLookups<'w, 's> {
    player: Query<'w, 's, &'static Transform, With<Player>>,
    interactables:
        Query<'w, 's, (Entity, &'static Transform, Option<&'static Children>), With<Interactable>>,
    pots: Query<'w, 's, &'static crate::components::pot::Pot>,
    highlights: Query<'w, 's, &'static HighlightMarker>,
    outlines: Query<'w, 's, &'static MeshMaterial2d<SpriteOutlineMaterial>>,
    growth_texts: Query<'w, 's, (Entity, &'static crate::plugins::farm::GrowthTimerText)>,
    visibility: Query<'w, 's, &'static mut Visibility>,
}

fn highlight_interactables_in_range(
    mut lookups: HighlightLookups,
    mut materials: Option<ResMut<Assets<SpriteOutlineMaterial>>>,
    time: Res<Time>,
    panels: OpenPanels,
    phase: Phase,
) {
    if !phase.is_farming() {
        // Nothing is interactable off the farm, so no crop should show its days.
        for (entity, _) in lookups.growth_texts.iter() {
            if let Ok(mut vis) = lookups.visibility.get_mut(entity) {
                *vis = Visibility::Hidden;
            }
        }
        return;
    }
    let Ok(player_transform) = lookups.player.single() else {
        return;
    };
    let player_pos = player_transform.translation.truncate();

    for (entity, transform, children) in lookups.interactables.iter() {
        let distance = player_pos.distance(transform.translation.truncate());

        let hidden = panels.any_open()
            || lookups
                .pots
                .get(entity)
                .map(|p| p.state == crate::components::pot::PotState::Watered)
                .unwrap_or(false);
        let active = !hidden && distance <= INTERACTION_RANGE;
        let highlight = if active { 1.0 } else { 0.0 };
        let now = time.elapsed_secs();

        // A pot's days-remaining number stays up once the pot is watered too (it
        // is no longer interactable then, but the growth is still worth reading),
        // so it only needs the player nearby with no panel in the way.
        let revealed = !panels.any_open() && distance <= INTERACTION_RANGE;
        if let Ok(pot) = lookups.pots.get(entity) {
            for (text_entity, marker) in lookups.growth_texts.iter() {
                if marker.pot_index == pot.index
                    && let Ok(mut vis) = lookups.visibility.get_mut(text_entity)
                {
                    *vis = if revealed {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
            }
        }

        // Anything with the shared outline material — pot mounds, farm props and
        // the plants growing on the pots — lights up by outlining its silhouette.
        // A pot and its plant each own a material, so both light together and the
        // ring ends up around the outermost edge of the soil and the plant.
        if let Some(materials) = materials.as_deref_mut() {
            if let Ok(handle) = lookups.outlines.get(entity)
                && let Some(mut material) = materials.get_mut(handle)
            {
                material.params.highlight = highlight;
                material.params.time = now;
            }
            for child in children.into_iter().flat_map(Children::iter) {
                if let Ok(handle) = lookups.outlines.get(child)
                    && let Some(mut material) = materials.get_mut(handle)
                {
                    material.params.highlight = highlight;
                    material.params.time = now;
                }
            }
        }

        // Everything else keeps the simple highlight child.
        let Some(children) = children else {
            continue;
        };
        for child in children.iter() {
            if lookups.highlights.get(child).is_ok() {
                if let Ok(mut vis) = lookups.visibility.get_mut(child) {
                    *vis = if active {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::InteractionType;
    use crate::materials::sprite_outline::SpriteOutlineParams;
    use crate::states::{DayPhase, GameState};
    use bevy::ecs::message::MessageReader;
    use bevy::state::app::StatesPlugin;
    use bevy::transform::TransformPlugin;

    #[derive(Resource, Default)]
    struct CapturedEvents(Vec<InteractionEvent>);

    fn capture_events(
        mut captured: ResMut<CapturedEvents>,
        mut reader: MessageReader<InteractionEvent>,
    ) {
        for event in reader.read() {
            captured.0.push(event.clone());
        }
    }

    fn setup_interaction_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<CraftingMenu>()
            .init_resource::<InventoryPanel>()
            .init_resource::<CropSelectMenu>()
            .init_resource::<CapturedEvents>()
            .add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_message::<InteractionEvent>()
            .add_systems(Update, player_proximity_interaction)
            .add_systems(Update, mouse_raycast_interaction)
            .add_systems(Update, highlight_interactables_in_range)
            .add_systems(
                Update,
                capture_events
                    .after(player_proximity_interaction)
                    .after(mouse_raycast_interaction),
            );

        app.world_mut()
            .spawn((Player, Transform::from_xyz(0.0, 0.0, 1.0)));

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);

        // Process state transitions
        app.update();
        app
    }

    fn spawn_interactable(app: &mut App, pos: Vec2, marker: impl Component) -> Entity {
        app.world_mut()
            .spawn((Interactable, marker, Transform::from_xyz(pos.x, pos.y, 0.0)))
            .with_children(|parent| {
                parent.spawn((
                    HighlightMarker,
                    Sprite {
                        color: Color::srgba(1.0, 1.0, 0.0, 0.5),
                        custom_size: Some(Vec2::splat(40.0 * 1.15)),
                        ..default()
                    },
                    Transform::from_xyz(0.0, 0.0, -0.1),
                    Visibility::Hidden,
                    Name::new("Highlight"),
                ));
            })
            .id()
    }

    fn get_captured_events(app: &mut App) -> Vec<InteractionEvent> {
        let captured = app.world().resource::<CapturedEvents>();
        captured.0.clone()
    }

    #[test]
    fn space_interacts_with_closest_in_range() {
        let mut app = setup_interaction_app();
        let _far = spawn_interactable(&mut app, Vec2::new(20.0, 0.0), FarmPot);
        let e2 = spawn_interactable(&mut app, Vec2::new(10.0, 0.0), FarmPot);
        app.update(); // Allow TransformPropagation to compute GlobalTransform

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].entity, e2);
        assert_eq!(events[0].interaction_type, InteractionType::FarmAction);
    }

    #[test]
    fn space_no_interaction_when_none_in_range() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(100.0, 0.0), FarmPot);
        app.update(); // Allow TransformPropagation to compute GlobalTransform

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn space_no_interaction_when_no_pots() {
        let mut app = setup_interaction_app();

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn space_does_nothing_while_the_crafting_menu_is_open() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(10.0, 0.0), FarmPot);
        app.world_mut().resource_mut::<CraftingMenu>().open = true;

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn highlights_hide_while_the_crafting_menu_is_open() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(20.0, 0.0), FarmPot);
        app.update();

        let highlight_entity = {
            let mut q = app.world_mut().query::<(Entity, &HighlightMarker)>();
            q.iter(app.world())
                .next()
                .expect("Highlight child not found")
                .0
        };
        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Visible
        );

        app.world_mut().resource_mut::<CraftingMenu>().open = true;
        app.update();

        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Hidden
        );
    }

    #[test]
    fn space_does_nothing_while_the_inventory_is_open() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(10.0, 0.0), FarmPot);
        app.world_mut()
            .resource_mut::<InventoryPanel>()
            .open_panel();

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn highlights_hide_while_the_inventory_is_open() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(20.0, 0.0), FarmPot);
        app.update();

        let highlight_entity = {
            let mut q = app.world_mut().query::<(Entity, &HighlightMarker)>();
            q.iter(app.world())
                .next()
                .expect("Highlight child not found")
                .0
        };
        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Visible
        );

        app.world_mut()
            .resource_mut::<InventoryPanel>()
            .open_panel();
        app.update();

        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Hidden
        );
    }

    #[test]
    fn highlights_come_back_after_the_inventory_closes() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(20.0, 0.0), FarmPot);
        let highlight_entity = {
            let mut q = app.world_mut().query::<(Entity, &HighlightMarker)>();
            q.iter(app.world())
                .next()
                .expect("Highlight child not found")
                .0
        };
        app.world_mut()
            .resource_mut::<InventoryPanel>()
            .open_panel();
        app.update();
        app.world_mut()
            .resource_mut::<InventoryPanel>()
            .close_panel();
        app.update();

        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Visible
        );
    }

    #[test]
    fn space_does_nothing_while_the_crop_picker_is_open() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(10.0, 0.0), FarmPot);
        app.world_mut()
            .resource_mut::<CropSelectMenu>()
            .open_menu(0);

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn highlights_hide_while_the_crop_picker_is_open() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(20.0, 0.0), FarmPot);
        app.update();

        let highlight_entity = {
            let mut q = app.world_mut().query::<(Entity, &HighlightMarker)>();
            q.iter(app.world())
                .next()
                .expect("Highlight child not found")
                .0
        };
        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Visible
        );

        app.world_mut()
            .resource_mut::<CropSelectMenu>()
            .open_menu(0);
        app.update();

        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Hidden
        );
    }

    #[test]
    fn space_only_in_farming_phase() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(10.0, 0.0), FarmPot);

        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossSelect);
        app.update();

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn space_only_in_playing_state() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(10.0, 0.0), FarmPot);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::LoadingAssets);
        app.update();

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn space_emits_correct_interaction_type() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(10.0, 0.0), FarmPot);
        spawn_interactable(&mut app, Vec2::new(15.0, 0.0), CraftingStation);
        app.update(); // Allow TransformPropagation to compute GlobalTransform

        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.press(KeyCode::Space);
        app.update();

        let events = get_captured_events(&mut app);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].interaction_type, InteractionType::FarmAction);
    }

    #[test]
    fn highlight_adds_outline_in_range() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(20.0, 0.0), FarmPot);
        app.update(); // Allow TransformPropagation

        // Find the highlight entity by marker and check its visibility
        let highlight_entity = {
            let mut q = app.world_mut().query::<(Entity, &HighlightMarker)>();
            q.iter(app.world())
                .next()
                .expect("Highlight child not found")
                .0
        };
        let has_outline = app
            .world()
            .get::<Visibility>(highlight_entity)
            .map(|v| *v == Visibility::Visible)
            .unwrap_or(false);
        assert!(has_outline);
    }

    #[test]
    fn highlight_removes_outline_out_of_range() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(100.0, 0.0), FarmPot);
        app.update(); // Allow TransformPropagation

        let highlight_entity = {
            let mut q = app.world_mut().query::<(Entity, &HighlightMarker)>();
            q.iter(app.world())
                .next()
                .expect("Highlight child not found")
                .0
        };
        let has_outline = app
            .world()
            .get::<Visibility>(highlight_entity)
            .map(|v| *v == Visibility::Visible)
            .unwrap_or(false);
        assert!(!has_outline);
    }

    #[test]
    fn highlight_updates_when_player_moves() {
        let mut app = setup_interaction_app();
        spawn_interactable(&mut app, Vec2::new(0.0, 0.0), FarmPot);
        app.update(); // Allow TransformPropagation

        let highlight_entity = {
            let mut q = app.world_mut().query::<(Entity, &HighlightMarker)>();
            q.iter(app.world())
                .next()
                .expect("Highlight child not found")
                .0
        };
        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Visible
        );

        let mut player_transform = *app
            .world_mut()
            .query_filtered::<&mut Transform, With<Player>>()
            .single(app.world_mut())
            .unwrap();
        player_transform.translation.x = 100.0;
        app.world_mut()
            .query_filtered::<&mut Transform, With<Player>>()
            .single_mut(app.world_mut())
            .unwrap()
            .translation = player_transform.translation;
        app.update();

        assert_eq!(
            app.world().get::<Visibility>(highlight_entity).unwrap(),
            &Visibility::Hidden
        );
    }

    #[test]
    fn highlight_only_on_interactables() {
        let mut app = setup_interaction_app();
        let entity = app
            .world_mut()
            .spawn((
                Transform::from_xyz(10.0, 0.0, 0.0),
                GlobalTransform::default(),
            ))
            .id();

        app.update();

        let has_outline = app.world().get::<Outline>(entity).is_some();
        assert!(!has_outline);
    }

    #[test]
    fn interactable_creation() {
        let interactable = Interactable::new();
        let _ = interactable;
    }

    #[test]
    fn interactable_default() {
        let interactable = Interactable;
        let _ = interactable;
    }

    fn setup_mound_highlight_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<CraftingMenu>()
            .init_resource::<InventoryPanel>()
            .init_resource::<CropSelectMenu>()
            .init_resource::<Assets<SpriteOutlineMaterial>>()
            .add_plugins((MinimalPlugins, TransformPlugin, StatesPlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_message::<InteractionEvent>()
            .add_systems(Update, highlight_interactables_in_range);

        app.world_mut()
            .spawn((Player, Transform::from_xyz(0.0, 0.0, 1.0)));
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);

        app.update();
        app
    }

    /// Spawns a mound-style interactable that carries its own material rather
    /// than a highlight child, and returns that material.
    fn spawn_mound(
        app: &mut App,
        pos: Vec2,
        state: crate::components::pot::PotState,
    ) -> Handle<SpriteOutlineMaterial> {
        let material = app
            .world_mut()
            .resource_mut::<Assets<SpriteOutlineMaterial>>()
            .add(SpriteOutlineMaterial::new(
                Handle::default(),
                SpriteOutlineParams::new(64.0),
            ));
        let mut pot = crate::components::pot::Pot::new(0);
        pot.state = state;
        app.world_mut().spawn((
            Interactable,
            FarmPot,
            pot,
            Transform::from_xyz(pos.x, pos.y, 0.0),
            MeshMaterial2d(material.clone()),
        ));
        material
    }

    fn highlight_of_material(app: &App, material: &Handle<SpriteOutlineMaterial>) -> f32 {
        app.world()
            .resource::<Assets<SpriteOutlineMaterial>>()
            .get(material)
            .expect("the mound material exists")
            .params
            .highlight
    }

    #[test]
    fn a_mound_material_lights_its_outline_when_the_player_is_in_range() {
        let mut app = setup_mound_highlight_app();
        let material = spawn_mound(
            &mut app,
            Vec2::new(10.0, 0.0),
            crate::components::pot::PotState::Empty,
        );
        app.update();

        assert_eq!(highlight_of_material(&app, &material), 1.0);
    }

    #[test]
    fn a_mound_material_goes_dark_when_the_player_is_out_of_range() {
        let mut app = setup_mound_highlight_app();
        let material = spawn_mound(
            &mut app,
            Vec2::new(1000.0, 0.0),
            crate::components::pot::PotState::Empty,
        );
        app.update();

        assert_eq!(highlight_of_material(&app, &material), 0.0);
    }

    #[test]
    fn a_watered_mound_never_lights_its_outline() {
        let mut app = setup_mound_highlight_app();
        let material = spawn_mound(
            &mut app,
            Vec2::new(10.0, 0.0),
            crate::components::pot::PotState::Watered,
        );
        app.update();

        assert_eq!(
            highlight_of_material(&app, &material),
            0.0,
            "watered soil is not interactable until the next day"
        );
    }

    /// Spawns a mound with a plant child that carries its own outline material,
    /// and returns both materials in `(soil, plant)` order.
    fn spawn_potted_plant(
        app: &mut App,
        pos: Vec2,
    ) -> (Handle<SpriteOutlineMaterial>, Handle<SpriteOutlineMaterial>) {
        let soil = app
            .world_mut()
            .resource_mut::<Assets<SpriteOutlineMaterial>>()
            .add(SpriteOutlineMaterial::new(
                Handle::default(),
                SpriteOutlineParams::new(64.0),
            ));
        let plant = app
            .world_mut()
            .resource_mut::<Assets<SpriteOutlineMaterial>>()
            .add(SpriteOutlineMaterial::new(
                Handle::default(),
                SpriteOutlineParams::new(1280.0),
            ));
        app.world_mut()
            .spawn((
                Interactable,
                FarmPot,
                crate::components::pot::Pot::new(0),
                Transform::from_xyz(pos.x, pos.y, 0.0),
                MeshMaterial2d(soil.clone()),
            ))
            .with_children(|parent| {
                parent.spawn((
                    crate::components::crop_sprite::CropSprite::new(
                        crate::components::pot::CropType::Starter,
                        crate::components::crop_sprite::CropStage::Seedling,
                    ),
                    Transform::from_xyz(0.0, 0.0, 0.5),
                    MeshMaterial2d(plant.clone()),
                ));
            });
        (soil, plant)
    }

    #[test]
    fn a_plant_child_lights_up_with_its_mound() {
        let mut app = setup_mound_highlight_app();
        let (soil, plant) = spawn_potted_plant(&mut app, Vec2::new(10.0, 0.0));
        app.update();

        assert_eq!(
            highlight_of_material(&app, &soil),
            1.0,
            "the soil outline lights up in range"
        );
        assert_eq!(
            highlight_of_material(&app, &plant),
            1.0,
            "the plant outline lights up with the soil so the ring covers both"
        );
    }

    #[test]
    fn a_plant_child_stays_dark_out_of_range() {
        let mut app = setup_mound_highlight_app();
        let (soil, plant) = spawn_potted_plant(&mut app, Vec2::new(1000.0, 0.0));
        app.update();

        assert_eq!(highlight_of_material(&app, &soil), 0.0);
        assert_eq!(highlight_of_material(&app, &plant), 0.0);
    }

    #[test]
    fn a_pots_days_number_shows_while_nearby_even_once_watered() {
        let mut app = setup_mound_highlight_app();
        let material = app
            .world_mut()
            .resource_mut::<Assets<SpriteOutlineMaterial>>()
            .add(SpriteOutlineMaterial::new(
                Handle::default(),
                SpriteOutlineParams::new(64.0),
            ));
        let pot = app
            .world_mut()
            .spawn((
                Interactable,
                FarmPot,
                crate::components::pot::Pot::new(3),
                Transform::from_xyz(10.0, 0.0, 0.0),
                MeshMaterial2d(material),
            ))
            .id();
        let text = app
            .world_mut()
            .spawn((
                crate::plugins::farm::GrowthTimerText { pot_index: 3 },
                Transform::default(),
                Visibility::Inherited,
            ))
            .id();

        app.update();
        assert_eq!(
            app.world().get::<Visibility>(text).unwrap(),
            &Visibility::Visible,
            "an in-range pot shows its days"
        );

        // Watering makes the pot non-interactable, but the number stays up.
        app.world_mut()
            .get_mut::<crate::components::pot::Pot>(pot)
            .unwrap()
            .state = crate::components::pot::PotState::Watered;
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(text).unwrap(),
            &Visibility::Visible,
            "a watered pot still shows its days"
        );

        let player = app
            .world_mut()
            .query_filtered::<Entity, With<Player>>()
            .single(app.world_mut())
            .unwrap();
        app.world_mut()
            .get_mut::<Transform>(player)
            .unwrap()
            .translation
            .x = 1000.0;
        app.update();
        assert_eq!(
            app.world().get::<Visibility>(text).unwrap(),
            &Visibility::Hidden,
            "walking away hides the number again"
        );
    }

    /// Spawns a farm prop (crafting station / boss door) carrying the shared
    /// outline material, and returns that material.
    fn spawn_prop(app: &mut App, pos: Vec2) -> Handle<SpriteOutlineMaterial> {
        let material = app
            .world_mut()
            .resource_mut::<Assets<SpriteOutlineMaterial>>()
            .add(SpriteOutlineMaterial::new(
                Handle::default(),
                SpriteOutlineParams::new(crate::plugins::prop::PROP_TEX_SIZE as f32),
            ));
        app.world_mut().spawn((
            Interactable,
            CraftingStation,
            Transform::from_xyz(pos.x, pos.y, 0.0),
            MeshMaterial2d(material.clone()),
        ));
        material
    }

    #[test]
    fn a_prop_lights_its_outline_when_the_player_is_in_range() {
        let mut app = setup_mound_highlight_app();
        let material = spawn_prop(&mut app, Vec2::new(10.0, 0.0));
        app.update();

        assert_eq!(highlight_of_material(&app, &material), 1.0);
    }

    #[test]
    fn a_prop_goes_dark_when_the_player_is_out_of_range() {
        let mut app = setup_mound_highlight_app();
        let material = spawn_prop(&mut app, Vec2::new(1000.0, 0.0));
        app.update();

        assert_eq!(highlight_of_material(&app, &material), 0.0);
    }
}
