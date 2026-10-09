use crate::components::collider::Collider;
use crate::components::pot::{Pot, PotState};
use crate::events::{CropHarvested, CropPlanted, CropWatered, DayAdvanced, InteractionEvent};
use crate::materials::sprite_outline::{SpriteOutlineMaterial, SpriteOutlineParams};
use crate::plugins::interaction::{FarmPot, Interactable};
use crate::resources::crop_select::CropSelectMenu;
use crate::resources::day_cycle::DayCycle;
use crate::resources::farm::{CropUnlocks, DayCounter, FarmState};
use crate::resources::inventory::Inventory;
use crate::resources::level::{LevelEntity, LevelSet};
use crate::states::{GameState, Phase};
use crate::utils::mound::mound_alpha_field;
use bevy::asset::RenderAssetUsages;
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite_render::{Material2dPlugin, MeshMaterial2d};

pub struct FarmPlugin;

impl Plugin for FarmPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DayCounter>()
            .init_resource::<CropUnlocks>()
            .init_resource::<FarmState>()
            .init_resource::<Inventory>()
            .init_resource::<CropSelectMenu>()
            .init_resource::<DayCycle>()
            .init_resource::<FarmAssets>()
            .init_resource::<MoundSources>()
            .add_message::<CropPlanted>()
            .add_message::<CropWatered>()
            .add_message::<CropHarvested>()
            .add_message::<DayAdvanced>()
            .add_systems(OnEnter(GameState::Playing), grab_mound_handles)
            .add_systems(OnExit(GameState::Playing), despawn_pots)
            .add_systems(
                Update,
                (build_mound_textures, attach_mound_visuals)
                    .chain()
                    .after(LevelSet::Load),
            )
            .add_systems(FixedUpdate, pot_interaction_handler)
            .add_systems(FixedUpdate, update_pot_visuals)
            .add_systems(Update, begin_next_day.after(LevelSet::Load))
            .add_systems(Update, debug_advance_day.after(LevelSet::Load))
            .add_systems(Update, harvest_into_inventory)
            .add_systems(
                Update,
                snapshot_pots.after(begin_next_day).after(debug_advance_day),
            );

        // A custom 2D material needs the asset infrastructure only the real
        // renderer brings; headless tests skip it and simply keep the mound's
        // gameplay components. The material backs the pot mounds and the farm
        // props alike, so it is registered here once for the whole game.
        if app.world().get_resource::<AssetServer>().is_some() {
            crate::materials::sprite_outline::register(app);
            app.add_plugins(Material2dPlugin::<SpriteOutlineMaterial>::default());
        }
    }
}

/// The collision footprint of a pot. The drawn mound is a little larger.
pub const POT_SIZE: f32 = 40.0;
/// Edge length the mound is drawn at, in world units.
pub const MOUND_SIZE: f32 = 48.0;
/// Side of the square mound texture the source art is baked down to.
pub const MOUND_TEX_SIZE: u32 = 64;

/// Z of a dry mound, below the crop sprite layer at [`CROP_SPRITE_Z`].
pub const MOUND_DRY_Z: f32 = 0.0;
/// Z of a wet mound, above the crop sprite so the soaked soil reads on top.
pub const MOUND_WET_Z: f32 = 1.0;
/// Where a crop's own (future) sprite sits, between the dry and wet mounds.
pub const CROP_SPRITE_Z: f32 = 0.5;

/// Marker component linking a growth timer Text2d to its pot by index.
#[derive(Component)]
pub struct GrowthTimerText {
    pub pot_index: usize,
}

/// The shared mound quad and the two baked mound textures.
///
/// Textures are baked from the square tile art on the CPU (resize + elliptical
/// mask), so every mound shares them. The quad is one handle reused by every pot.
#[derive(Resource, Default)]
pub struct FarmAssets {
    quad: Option<Handle<Mesh>>,
    dry: Option<Handle<Image>>,
    wet: Option<Handle<Image>>,
}

impl FarmAssets {
    fn quad(&mut self, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
        self.quad
            .get_or_insert_with(|| meshes.add(Rectangle::new(MOUND_SIZE, MOUND_SIZE)))
            .clone()
    }
}

/// Handles to the raw tile art the mound textures are baked from.
#[derive(Resource, Default)]
pub struct MoundSources {
    dry: Option<Handle<Image>>,
    wet: Option<Handle<Image>>,
}

/// Spawns one pot at a world position. Position comes from the level's `p`
/// markers, and the index is the marker's order in the file.
///
/// Only the gameplay components are attached here. The mound graphic is applied
/// later by [`attach_mound_visuals`], which waits for its textures to bake.
/// The growth timer text is spawned as a separate entity (not a child) to avoid
/// Bevy hierarchy warnings (B0004) with Text2d.
///
/// If `level_tag` is provided, both the pot and its growth timer text are
/// tagged so they are despawned together when the level is unloaded.
pub fn spawn_pot(
    commands: &mut Commands,
    index: usize,
    position: Vec2,
    level_tag: Option<LevelEntity>,
) -> Entity {
    let mut pot_entity = commands.spawn((
        Pot::new(index),
        Collider {
            size: Vec2::splat(POT_SIZE),
            is_solid: true,
        },
        Interactable::new(),
        FarmPot,
        Transform::from_xyz(position.x, position.y, MOUND_DRY_Z),
        Name::new(format!("Pot {}", index)),
    ));

    if let Some(tag) = level_tag {
        pot_entity.insert(tag);
    }
    let pot_entity = pot_entity.id();

    // Spawn growth timer text as a sibling entity with a marker component
    // so we can find it in update_pot_visuals without a parent-child relationship.
    let mut text_entity = commands.spawn((
        GrowthTimerText { pot_index: index },
        Text2d::new(""),
        TextFont {
            font_size: FontSize::Px(16.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Transform::from_xyz(position.x, position.y + POT_SIZE * 0.6, CROP_SPRITE_Z + 0.1),
        Name::new(format!("Growth Timer Text {}", index)),
    ));

    if let Some(tag) = level_tag {
        text_entity.insert(tag);
    }

    pot_entity
}

/// The mound texture a pot should show in `state`, preferring wet art when the
/// soil is watered and falling back to the dry art if the wet texture is missing.
fn mound_texture(assets: &FarmAssets, state: PotState, dry: &Handle<Image>) -> Handle<Image> {
    if state == PotState::Watered {
        assets.wet.clone().unwrap_or_else(|| dry.clone())
    } else {
        dry.clone()
    }
}

/// Requests the raw dirt art once, when the farm first loads.
fn grab_mound_handles(server: Option<Res<AssetServer>>, mut sources: ResMut<MoundSources>) {
    if sources.dry.is_some() {
        return;
    }
    let Some(server) = server else {
        return;
    };
    sources.dry = Some(server.load("images/tiles/dirt.png"));
    sources.wet = Some(server.load("images/tiles/dirt_wet.png"));
}

/// Bakes the square tile art into masked, downscaled mound textures.
fn build_mound_textures(
    sources: Res<MoundSources>,
    server: Option<Res<AssetServer>>,
    images: Option<ResMut<Assets<Image>>>,
    mut assets: ResMut<FarmAssets>,
) {
    if assets.dry.is_some() && assets.wet.is_some() {
        return;
    }
    let (Some(server), Some(mut images)) = (server, images) else {
        return;
    };
    let (Some(dry_handle), Some(wet_handle)) = (sources.dry.as_ref(), sources.wet.as_ref()) else {
        return;
    };
    if !is_loaded(&server, dry_handle) || !is_loaded(&server, wet_handle) {
        return;
    }

    if assets.dry.is_none()
        && let Some(source) = images.get(dry_handle)
    {
        let baked = mound_image_from(source);
        assets.dry = Some(images.add(baked));
        log::info!("baked dry mound texture from images/tiles/dirt.png");
    }
    if assets.wet.is_none()
        && let Some(source) = images.get(wet_handle)
    {
        let baked = mound_image_from(source);
        assets.wet = Some(images.add(baked));
        log::info!("baked wet mound texture from images/tiles/dirt_wet.png");
    }
}

fn is_loaded(server: &AssetServer, handle: &Handle<Image>) -> bool {
    matches!(
        server.get_load_state(handle.id()),
        Some(bevy::asset::LoadState::Loaded)
    )
}

/// Downscales one source image and multiplies the mound mask into its alpha.
fn mound_image_from(source: &Image) -> Image {
    let width = source.width();
    let height = source.height();
    let data = source.data.as_ref().expect("image data should exist");
    let rgba =
        image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_raw(width, height, data.clone())
            .expect("valid rgba data");
    let resized = image::DynamicImage::ImageRgba8(rgba)
        .resize_exact(
            MOUND_TEX_SIZE,
            MOUND_TEX_SIZE,
            image::imageops::FilterType::Lanczos3,
        )
        .to_rgba8();

    let alpha = mound_alpha_field(MOUND_TEX_SIZE);
    let mut pixels = resized.into_raw();
    for (index, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let mask = alpha.get(index).copied().unwrap_or(1.0);
        pixel[3] = ((pixel[3] as f32) * mask).round() as u8;
    }

    let mut image = Image::new(
        Extent3d {
            width: MOUND_TEX_SIZE,
            height: MOUND_TEX_SIZE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::linear();
    image
}

/// Gives every visual-less pot its mound mesh and material once the textures are
/// baked. Each pot owns a material instance so its outline can light up alone.
fn attach_mound_visuals(
    mut commands: Commands,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<SpriteOutlineMaterial>>>,
    mut assets: ResMut<FarmAssets>,
    mut pots: Query<(Entity, &Pot, &mut Transform), Without<Mesh2d>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    let Some(dry) = assets.dry.clone() else {
        return;
    };
    let quad = assets.quad(&mut meshes);

    let mut attached = 0usize;
    for (entity, pot, mut transform) in pots.iter_mut() {
        transform.translation.z = if pot.state == PotState::Watered {
            MOUND_WET_Z
        } else {
            MOUND_DRY_Z
        };
        let texture = mound_texture(&assets, pot.state, &dry);
        let params = SpriteOutlineParams::new(MOUND_TEX_SIZE as f32);
        let material = materials.add(SpriteOutlineMaterial::new(texture, params));
        commands.entity(entity).insert((
            Mesh2d(quad.clone()),
            MeshMaterial2d(material),
            Name::new(format!("Mound {}", pot.index)),
        ));
        attached += 1;
    }
    if attached > 0 {
        log::info!("attached mound visuals to {attached} pots");
    }
}

fn despawn_pots(
    mut commands: Commands,
    pots: Query<Entity, With<Pot>>,
    texts: Query<Entity, With<GrowthTimerText>>,
) {
    for entity in pots.iter() {
        commands.entity(entity).despawn();
    }
    for entity in texts.iter() {
        commands.entity(entity).despawn();
    }
}

fn pot_interaction_handler(
    mut events: MessageReader<InteractionEvent>,
    mut pots: Query<&mut Pot>,
    mut crop_select: ResMut<CropSelectMenu>,
    mut watered_events: MessageWriter<CropWatered>,
    mut harvested_events: MessageWriter<CropHarvested>,
    phase: Phase,
) {
    // Drain every event even outside farming so clicks made during a boss fight
    // can never be replayed against the pots once the farm is back.
    for event in events.read() {
        if !phase.is_farming() {
            continue;
        }
        let Ok(mut pot) = pots.get_mut(event.entity) else {
            continue;
        };

        match pot.state {
            PotState::Empty => {
                // Let the player choose which unlocked crop to plant.
                crop_select.open_menu(pot.index);
            }
            PotState::Planted => {
                if pot.water() {
                    watered_events.write(CropWatered);
                }
            }
            PotState::Watered => {
                // Already watered today - no action needed
            }
            PotState::Ready => {
                if let Some(crop) = pot.harvest() {
                    harvested_events.write(CropHarvested(crop));
                }
            }
        }
    }
}

fn harvest_into_inventory(
    mut events: MessageReader<CropHarvested>,
    mut inventory: ResMut<Inventory>,
) {
    for event in events.read() {
        inventory.add_crop(event.0, 1);
    }
}

fn begin_next_day(
    mut pots: Query<&mut Pot>,
    mut day_counter: ResMut<DayCounter>,
    mut day_cycle: ResMut<DayCycle>,
    mut day_advanced_events: MessageWriter<DayAdvanced>,
    phase: Phase,
) {
    if !phase.is_farming() || !day_cycle.pending_advance {
        return;
    }
    day_cycle.pending_advance = false;
    advance_day(&mut pots, &mut day_counter, &mut day_advanced_events);
}

fn debug_advance_day(
    mut pots: Query<&mut Pot>,
    mut day_counter: ResMut<DayCounter>,
    mut day_advanced_events: MessageWriter<DayAdvanced>,
    keys: Res<ButtonInput<KeyCode>>,
    phase: Phase,
) {
    if !phase.is_farming() {
        return;
    }
    if cfg!(debug_assertions) && keys.just_pressed(KeyCode::F9) {
        advance_day(&mut pots, &mut day_counter, &mut day_advanced_events);
    }
}

fn advance_day(
    pots: &mut Query<&mut Pot>,
    day_counter: &mut DayCounter,
    day_advanced_events: &mut MessageWriter<DayAdvanced>,
) {
    for mut pot in pots.iter_mut() {
        pot.advance_day();
        log::debug!(
            "pot {}: {:?} ({} days left)",
            pot.index,
            pot.state,
            pot.days_remaining
        );
    }
    day_counter.advance();
    log::info!("day advanced to {}", day_counter.0);
    day_advanced_events.write(DayAdvanced { day: day_counter.0 });
}

#[allow(clippy::type_complexity)]
fn update_pot_visuals(
    mut pots: Query<
        (
            &Pot,
            &mut Transform,
            Option<&MeshMaterial2d<SpriteOutlineMaterial>>,
        ),
        Without<GrowthTimerText>,
    >,
    mut materials: Option<ResMut<Assets<SpriteOutlineMaterial>>>,
    assets: Res<FarmAssets>,
    mut text_query: Query<(&mut Text2d, &mut Transform, &GrowthTimerText), Without<Pot>>,
    phase: Phase,
) {
    if !phase.is_farming() {
        return;
    }
    let Some(dry) = assets.dry.clone() else {
        return;
    };

    for (pot, mut transform, material) in pots.iter_mut() {
        // Dry soil sits under the crop sprite; soaked soil reads on top of it.
        transform.translation.z = if pot.state == PotState::Watered {
            MOUND_WET_Z
        } else {
            MOUND_DRY_Z
        };

        if let (Some(materials), Some(handle)) = (materials.as_deref_mut(), material)
            && let Some(mut material) = materials.get_mut(handle)
        {
            let desired = mound_texture(&assets, pot.state, &dry);
            if material.texture != desired {
                material.texture = desired;
            }
        }

        // Update the growth timer text for this pot
        for (mut text, mut text_transform, marker) in text_query.iter_mut() {
            if marker.pot_index == pot.index {
                // Keep text position synced with pot (in case pot moves)
                text_transform.translation.x = transform.translation.x;
                text_transform.translation.y = transform.translation.y + POT_SIZE * 0.6;

                if pot.state == PotState::Empty || pot.days_remaining == 0 {
                    text.0.clear();
                } else {
                    text.0 = pot.days_remaining.to_string();
                }
                break;
            }
        }
    }
}

fn snapshot_pots(pots: Query<&Pot>, mut farm: ResMut<FarmState>) {
    if pots.is_empty() {
        return;
    }
    let mut snapshot: Vec<Pot> = pots.iter().copied().collect();
    snapshot.sort_by_key(|pot| pot.index);
    farm.pots = snapshot;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::pot::CropType;
    use crate::events::InteractionType;
    use crate::levels::LevelId;
    use crate::resources::level::LevelRequest;
    use crate::states::DayPhase;
    use bevy::state::app::StatesPlugin;
    use bevy::transform::TransformPlugin;

    const POT_COUNT: usize = 9;

    /// A bare app that runs the pot interaction handler in `Update` so tests do
    /// not depend on the fixed timestep ticking.
    fn setup_handler_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<CropSelectMenu>()
            .init_resource::<CropUnlocks>()
            .add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            .add_message::<InteractionEvent>()
            .add_message::<CropWatered>()
            .add_message::<CropHarvested>()
            .add_systems(Update, pot_interaction_handler);
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);
        app.update();
        app
    }

    fn interact(app: &mut App, entity: Entity) {
        app.world_mut().write_message(InteractionEvent {
            entity,
            interaction_type: InteractionType::FarmAction,
        });
        app.update();
    }

    #[test]
    fn interacting_with_an_empty_pot_opens_the_crop_picker_instead_of_planting() {
        let mut app = setup_handler_app();
        let pot = app.world_mut().spawn(Pot::new(2)).id();

        interact(&mut app, pot);

        let menu = app.world().resource::<CropSelectMenu>();
        assert!(menu.open, "the picker must open on an empty pot");
        assert_eq!(menu.pending_pot, 2);
        assert_eq!(
            app.world().get::<Pot>(pot).unwrap().state,
            PotState::Empty,
            "nothing is planted until the player chooses"
        );
    }

    #[test]
    fn interacting_with_a_planted_pot_still_waters_it() {
        let mut app = setup_handler_app();
        let pot = app.world_mut().spawn(Pot::new(0)).id();
        app.world_mut()
            .get_mut::<Pot>(pot)
            .unwrap()
            .plant(CropType::Starter);

        interact(&mut app, pot);

        assert_eq!(
            app.world().get::<Pot>(pot).unwrap().state,
            PotState::Watered
        );
        assert!(!app.world().resource::<CropSelectMenu>().open);
    }

    #[test]
    fn stale_boss_fight_clicks_do_not_replay_on_the_farm() {
        let mut app = setup_handler_app();
        let pot = app.world_mut().spawn(Pot::new(0)).id();

        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossFight);
        app.update();

        app.world_mut().write_message(InteractionEvent {
            entity: pot,
            interaction_type: InteractionType::FarmAction,
        });
        app.update();

        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::Farming);
        app.update();

        assert_eq!(
            app.world().get::<Pot>(pot).unwrap().state,
            PotState::Empty,
            "a click made during the boss fight must not affect the pot later"
        );
        assert!(
            !app.world().resource::<CropSelectMenu>().open,
            "the picker must not open from a stale click"
        );
    }

    fn setup_farm_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .add_plugins((
                MinimalPlugins,
                TransformPlugin,
                StatesPlugin,
                crate::plugins::level::LevelPlugin,
                FarmPlugin,
            ))
            .init_state::<GameState>()
            .init_state::<DayPhase>()
            // `FarmPlugin`'s interaction handler reads this message; in the game
            // `InteractionPlugin` registers it, but this bare test app must too.
            .add_message::<InteractionEvent>();
        app
    }

    fn enter_playing(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
    }

    fn exit_playing(app: &mut App) {
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Victory);
        app.update();
    }

    fn request_level(app: &mut App, id: LevelId) {
        app.world_mut().resource_mut::<LevelRequest>().0 = Some(id);
        app.update();
    }

    fn pot_indices(app: &mut App) -> Vec<usize> {
        let mut query = app.world_mut().query::<&Pot>();
        let mut indices: Vec<usize> = query.iter(app.world()).map(|pot| pot.index).collect();
        indices.sort_unstable();
        indices
    }

    #[test]
    fn farm_plugin_exists() {
        let _plugin = FarmPlugin;
    }

    #[test]
    fn pots_spawn_once_on_playing() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        assert_eq!(
            pot_indices(&mut app),
            (0..POT_COUNT).collect::<Vec<usize>>()
        );
    }

    #[test]
    fn pots_despawn_when_leaving_playing() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        assert_eq!(pot_indices(&mut app).len(), POT_COUNT);

        exit_playing(&mut app);
        assert!(pot_indices(&mut app).is_empty());
    }

    #[test]
    fn pots_do_not_duplicate_when_reentering_playing() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        exit_playing(&mut app);
        enter_playing(&mut app);

        assert_eq!(
            pot_indices(&mut app),
            (0..POT_COUNT).collect::<Vec<usize>>(),
            "re-entering Playing must not leave stale pots behind"
        );
    }

    #[test]
    fn pots_survive_day_phase_changes() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(DayPhase::BossSelect);
        app.update();

        assert_eq!(pot_indices(&mut app).len(), POT_COUNT);
    }

    #[test]
    fn a_fresh_farm_starts_with_empty_pots() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        let farm = app.world().resource::<FarmState>();
        assert_eq!(farm.pots.len(), POT_COUNT);
        assert!(farm.pots.iter().all(|pot| pot.state == PotState::Empty));
    }

    #[test]
    fn the_farm_state_tracks_planted_and_watered_pots() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        let pot = pot_entity(&mut app, 0);
        {
            let mut planted = app.world_mut().get_mut::<Pot>(pot).expect("pot entity");
            planted.plant(CropType::Starter);
            planted.water();
        }
        app.update();

        let farm = app.world().resource::<FarmState>();
        assert_eq!(farm.pots.len(), POT_COUNT);
        assert_eq!(farm.pots[0].state, PotState::Watered);
        assert!(farm.pots[0].watered_today);
        assert_eq!(farm.pots[1].state, PotState::Empty);
    }

    #[test]
    fn planted_crops_survive_a_boss_fight_round_trip() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        let pot = pot_entity(&mut app, 0);
        app.world_mut()
            .get_mut::<Pot>(pot)
            .expect("pot entity")
            .plant(CropType::CropA);
        app.update();

        request_level(&mut app, LevelId::ArenaA);
        assert!(pot_indices(&mut app).is_empty(), "the farm is left behind");

        request_level(&mut app, LevelId::Farm);
        let restored_index = pot_entity(&mut app, 0);
        let restored = app
            .world()
            .get::<Pot>(restored_index)
            .expect("pot is respawned");
        assert_eq!(restored.state, PotState::Watered);
        assert_eq!(restored.crop_type, CropType::CropA);
        assert_eq!(restored.days_remaining, CropType::CropA.growth_days());
    }

    #[test]
    fn watered_crops_advance_a_day_when_returning_from_a_boss_fight() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        let pot = pot_entity(&mut app, 0);
        {
            let mut planted = app.world_mut().get_mut::<Pot>(pot).expect("pot entity");
            planted.plant(CropType::CropA); // 2-day crop
            planted.water();
        }
        app.update();

        request_level(&mut app, LevelId::ArenaA);
        app.world_mut().resource_mut::<DayCycle>().request_advance();
        request_level(&mut app, LevelId::Farm);

        let restored_index = pot_entity(&mut app, 0);
        let restored = app
            .world()
            .get::<Pot>(restored_index)
            .expect("pot is respawned");
        assert_eq!(restored.state, PotState::Planted);
        assert_eq!(restored.days_remaining, CropType::CropA.growth_days() - 1);
        assert!(!restored.watered_today);
    }

    #[test]
    fn harvested_pots_are_still_empty_after_returning_to_the_farm() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        let pot = grow_pot_to_ready(&mut app, 0);
        let crop = app
            .world_mut()
            .get_mut::<Pot>(pot)
            .expect("pot entity")
            .harvest()
            .expect("ripe pot yields a crop");
        app.world_mut().write_message(CropHarvested(crop));
        app.update();

        request_level(&mut app, LevelId::ArenaB);
        request_level(&mut app, LevelId::Farm);

        let restored_index = pot_entity(&mut app, 0);
        assert_eq!(
            app.world()
                .get::<Pot>(restored_index)
                .expect("pot is respawned")
                .state,
            PotState::Empty
        );
    }

    #[test]
    fn harvesting_a_ripe_pot_stocks_the_inventory() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        let pot = grow_pot_to_ready(&mut app, 0);
        let crop = app
            .world_mut()
            .get_mut::<Pot>(pot)
            .expect("pot entity")
            .harvest()
            .expect("ripe pot yields a crop");

        app.world_mut().write_message(CropHarvested(crop));
        app.update();

        let inventory = app.world().resource::<Inventory>();
        assert_eq!(inventory.crop_count(CropType::Starter), 1);
        assert_eq!(app.world().get::<Pot>(pot).unwrap().state, PotState::Empty);
    }

    #[test]
    fn harvests_accumulate_in_the_inventory() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        for _ in 0..3 {
            app.world_mut()
                .write_message(CropHarvested(CropType::Starter));
        }
        for _ in 0..2 {
            app.world_mut()
                .write_message(CropHarvested(CropType::CropA));
        }
        app.update();

        let inventory = app.world().resource::<Inventory>();
        assert_eq!(inventory.crop_count(CropType::Starter), 3);
        assert_eq!(inventory.crop_count(CropType::CropA), 2);
        assert_eq!(inventory.crop_count(CropType::CropB), 0);
    }

    #[test]
    fn harvesting_an_unripe_pot_yields_nothing_to_stock() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        let pot = pot_entity(&mut app, 1);
        app.world_mut()
            .get_mut::<Pot>(pot)
            .expect("pot 1 exists")
            .plant(CropType::Starter);

        let crop = app.world_mut().get_mut::<Pot>(pot).unwrap().harvest();
        assert_eq!(crop, None);
        app.update();

        assert_eq!(
            app.world()
                .resource::<Inventory>()
                .crop_count(CropType::Starter),
            0
        );
    }

    fn pot_entity(app: &mut App, index: usize) -> Entity {
        let mut query = app.world_mut().query_filtered::<Entity, With<Pot>>();
        query
            .iter(app.world())
            .find(|entity| {
                app.world()
                    .get::<Pot>(*entity)
                    .is_some_and(|pot| pot.index == index)
            })
            .expect("pot entity")
    }

    fn grow_pot_to_ready(app: &mut App, index: usize) -> Entity {
        let pot = pot_entity(app, index);
        let mut pots = app.world_mut().get_mut::<Pot>(pot).expect("pot entity");
        pots.plant(CropType::Starter);
        for _ in 0..CropType::Starter.growth_days() {
            pots.water();
            pots.advance_day();
        }
        pot
    }

    #[test]
    fn baked_mound_art_is_solid_in_the_middle_and_clear_at_the_corners() {
        let source = Image::new(
            Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            vec![255u8; 16 * 16 * 4],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );

        let baked = mound_image_from(&source);
        let data = baked.data.as_ref().expect("baked pixels");
        let size = MOUND_TEX_SIZE as usize;
        assert_eq!(baked.width(), MOUND_TEX_SIZE);
        assert_eq!(baked.height(), MOUND_TEX_SIZE);

        let center = ((size / 2) * size + size / 2) * 4;
        assert_eq!(data[center + 3], 255, "the mound centre stays opaque");
        assert_eq!(data[3], 0, "the top-left corner fades out");
    }

    #[test]
    fn a_watered_pot_prefers_the_wet_mound_texture() {
        let dry = Handle::<Image>::default();
        let wet = Handle::<Image>::default();
        let assets = FarmAssets {
            quad: None,
            dry: Some(dry.clone()),
            wet: Some(wet.clone()),
        };

        assert_eq!(mound_texture(&assets, PotState::Empty, &dry), dry);
        assert_eq!(mound_texture(&assets, PotState::Watered, &dry), wet);
    }

    #[test]
    fn a_missing_wet_texture_falls_back_to_the_dry_one() {
        let dry = Handle::<Image>::default();
        let assets = FarmAssets {
            quad: None,
            dry: Some(dry.clone()),
            wet: None,
        };

        assert_eq!(mound_texture(&assets, PotState::Watered, &dry), dry);
    }
}
