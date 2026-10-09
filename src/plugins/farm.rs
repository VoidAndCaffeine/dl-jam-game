use crate::components::collider::Collider;
use crate::components::crop_sprite::{CROP_FRAME_COUNT, CROP_SPRITE_SIZE, CropSprite};
use crate::components::player_sprite::{FRAME_COLUMNS, FRAME_SIZE};
use crate::components::pot::{Pot, PotState};
use crate::events::{CropHarvested, CropPlanted, CropWatered, DayAdvanced, InteractionEvent};
use crate::materials::sprite_outline::{SpriteOutlineMaterial, SpriteOutlineParams, atlas_rect};
use crate::plugins::interaction::{FarmPot, Interactable};
use crate::resources::crop_select::CropSelectMenu;
use crate::resources::crop_sprite::{CropSpriteAssets, CropSpriteKey};
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
use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};

pub struct FarmPlugin;

impl Plugin for FarmPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DayCounter>()
            .init_resource::<CropUnlocks>()
            .init_resource::<FarmState>()
            .init_resource::<Inventory>()
            .init_resource::<CropSelectMenu>()
            .init_resource::<CropSpriteAssets>()
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
            // The plant child is spawned from the pot's growth state, then given
            // its sprite once the renderer's assets exist. The visual steps run
            // outside the farming check so a plant keeps animating on the farm
            // during boss select.
            .add_systems(
                Update,
                sync_crop_sprites
                    .after(LevelSet::Load)
                    .after(begin_next_day)
                    .after(debug_advance_day),
            )
            .add_systems(
                Update,
                (attach_crop_visuals, animate_crop_sprites)
                    .chain()
                    .after(sync_crop_sprites),
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

/// Z of the soil mound. Dry and wet soil share the pot's one mound quad, so a
/// single layer holds both.
pub const MOUND_Z: f32 = 0.0;
/// Where a crop's sprite sits, on top of the soil it is planted in.
pub const CROP_SPRITE_Z: f32 = 0.5;
/// How far a crop is lifted off its pot at its full scale. The plant art carries
/// its own soil in the lower half of the frame, so centring it on the pot sinks
/// it into the ground; half a tile brings the soil line back up to the mound. A
/// smaller crop is lifted proportionally less.
pub const CROP_VERTICAL_OFFSET: f32 = 16.0;
/// Y of the days-remaining text from its pot. The plant rises above the soil, so
/// the number sits down at the soil line instead of over the plant.
pub const GROWTH_TEXT_OFFSET: f32 = -8.0;
/// Z of the days-remaining text, above the plant so it stays legible.
pub const GROWTH_TEXT_Z: f32 = 1.5;

/// Side of the square crop sheet the plant art is drawn from: a 5x5 grid of
/// [`FRAME_SIZE`] frames.
pub const CROP_SHEET_SIZE: u32 = FRAME_SIZE * FRAME_COLUMNS;
/// Outline reach for a crop, in sheet texels. Tuned so a plant's ring reads at
/// the same world thickness as the 64px mound default.
pub const CROP_OUTLINE_WIDTH: f32 = 7.5;
/// The tint a crop is drawn with while its soil is soaked. The plant art bakes
/// its own soil, so this cool cast stands in for the wet mound it covers.
pub const WATERED_TINT: Vec4 = Vec4::new(0.55, 0.72, 1.0, 1.0);

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
    crop_quad: Option<Handle<Mesh>>,
    dry: Option<Handle<Image>>,
    wet: Option<Handle<Image>>,
}

impl FarmAssets {
    fn quad(&mut self, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
        self.quad
            .get_or_insert_with(|| meshes.add(Rectangle::new(MOUND_SIZE, MOUND_SIZE)))
            .clone()
    }

    /// The quad a crop sprite is drawn on, sized to [`CROP_SPRITE_SIZE`].
    fn crop_quad(&mut self, meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
        self.crop_quad
            .get_or_insert_with(|| meshes.add(Rectangle::new(CROP_SPRITE_SIZE, CROP_SPRITE_SIZE)))
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
        Transform::from_xyz(position.x, position.y, MOUND_Z),
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
        Transform::from_xyz(position.x, position.y + GROWTH_TEXT_OFFSET, GROWTH_TEXT_Z),
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
    pots: Query<(Entity, &Pot), Without<Mesh2d>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        return;
    };
    let Some(dry) = assets.dry.clone() else {
        return;
    };
    let quad = assets.quad(&mut meshes);

    let mut attached = 0usize;
    for (entity, pot) in pots.iter() {
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
    pots: Query<
        (
            &Pot,
            &Transform,
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

    for (pot, transform, material) in pots.iter() {
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
                text_transform.translation.y = transform.translation.y + GROWTH_TEXT_OFFSET;

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

/// The frame phase a plant in pot `pot_index` starts at, so two plants of the
/// same crop do not sway in lockstep. Seeded from the pot index, so it is stable
/// for that pot and needs no shared RNG.
fn crop_phase(pot_index: usize) -> usize {
    let mut rng = SmallRng::seed_from_u64(pot_index as u64 ^ 0x9E37_79B9_7F4A_7C15);
    rng.random_range(0..CROP_FRAME_COUNT)
}

/// Gives every planted pot a crop child and clears it from a pot that was
/// harvested. The child is a plain entity here; [`attach_crop_visuals`] dresses
/// it once the renderer's assets exist, exactly like the pot mounds.
fn sync_crop_sprites(
    mut commands: Commands,
    pots: Query<(Entity, &Pot, Option<&Children>)>,
    mut sprites: Query<&mut CropSprite>,
    phase: Phase,
) {
    if !phase.is_farming() {
        return;
    }
    for (entity, pot, children) in pots.iter() {
        let planted = pot.state != PotState::Empty;
        let child = children.and_then(|kids| kids.iter().find(|child| sprites.contains(*child)));
        match (planted, child) {
            (true, Some(child)) => {
                if let Ok(mut sprite) = sprites.get_mut(child) {
                    sprite.set_stage(pot.crop_type.stage_for_days_remaining(pot.days_remaining));
                    sprite.watered = pot.watered_today;
                }
            }
            (true, None) => {
                let stage = pot.crop_type.stage_for_days_remaining(pot.days_remaining);
                let mut crop =
                    CropSprite::new(pot.crop_type, stage).with_phase(crop_phase(pot.index));
                crop.watered = pot.watered_today;
                let index = pot.index;
                let scale = pot.crop_type.sprite_scale();
                commands.entity(entity).with_children(|parent| {
                    parent.spawn((
                        crop,
                        Transform::from_xyz(0.0, CROP_VERTICAL_OFFSET * scale, CROP_SPRITE_Z)
                            .with_scale(Vec3::splat(scale)),
                        Name::new(format!("Crop {}", index)),
                    ));
                });
            }
            (false, Some(child)) => {
                commands.entity(child).despawn();
            }
            (false, None) => {}
        }
    }
}

/// Gives every visual-less crop child its quad and outline material. Each plant
/// owns its own material so its outline can light up with its pot.
fn attach_crop_visuals(
    mut commands: Commands,
    server: Option<Res<AssetServer>>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<SpriteOutlineMaterial>>>,
    mut farm: ResMut<FarmAssets>,
    mut cache: ResMut<CropSpriteAssets>,
    crops: Query<(Entity, &CropSprite), Without<Mesh2d>>,
) {
    if crops.is_empty() {
        return;
    }
    let (Some(server), Some(mut meshes), Some(mut materials)) = (server, meshes, materials) else {
        return;
    };

    let quad = farm.crop_quad(&mut meshes);
    let mut attached = 0usize;
    for (entity, crop) in crops.iter() {
        let image = cache.image_for(CropSpriteKey::new(crop.crop, crop.stage), &server);
        let mut params =
            SpriteOutlineParams::atlas(CROP_SHEET_SIZE as f32, FRAME_COLUMNS, crop.frame);
        params.outline_width = CROP_OUTLINE_WIDTH;
        let material = materials.add(SpriteOutlineMaterial::new(image, params));
        commands
            .entity(entity)
            .insert((Mesh2d(quad.clone()), MeshMaterial2d(material)));
        attached += 1;
    }
    if attached > 0 {
        log::info!("attached crop visuals to {attached} plants");
    }
}

/// Advances every plant's clip and points its material at the current frame,
/// swapping the sheet when the crop grows into a new stage.
fn animate_crop_sprites(
    time: Res<Time>,
    server: Option<Res<AssetServer>>,
    mut materials: Option<ResMut<Assets<SpriteOutlineMaterial>>>,
    mut cache: ResMut<CropSpriteAssets>,
    mut crops: Query<(&mut CropSprite, &MeshMaterial2d<SpriteOutlineMaterial>)>,
) {
    let Some(server) = server else {
        return;
    };
    let dt = time.delta_secs();
    for (mut crop, handle) in crops.iter_mut() {
        crop.advance(dt);
        let image = cache.image_for(CropSpriteKey::new(crop.crop, crop.stage), server.as_ref());
        if let Some(materials) = materials.as_deref_mut()
            && let Some(mut material) = materials.get_mut(handle)
        {
            if material.texture != image {
                material.texture = image;
            }
            material.params.uv_rect = atlas_rect(FRAME_COLUMNS, crop.atlas_frame());
            let tint = if crop.watered {
                WATERED_TINT
            } else {
                Vec4::ONE
            };
            if material.params.tint != tint {
                material.params.tint = tint;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::crop_sprite::CropStage;
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

    /// The plant child of `pot`, if it has grown one yet.
    fn crop_child(app: &mut App, pot: Entity) -> Option<Entity> {
        let children: Vec<Entity> = app
            .world()
            .get::<Children>(pot)
            .map(|kids| kids.iter().collect())
            .unwrap_or_default();
        children
            .into_iter()
            .find(|child| app.world().get::<CropSprite>(*child).is_some())
    }

    #[test]
    fn planting_a_crop_grows_a_plant_child() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        let pot = pot_entity(&mut app, 0);

        app.world_mut()
            .get_mut::<Pot>(pot)
            .unwrap()
            .plant(CropType::CropA);
        app.update();

        let child = crop_child(&mut app, pot).expect("a plant is spawned");
        let crop = app.world().get::<CropSprite>(child).unwrap();
        assert_eq!(crop.crop, CropType::CropA);
        assert_eq!(
            crop.stage,
            CropStage::Seedling,
            "a fresh planting is a seedling"
        );
    }

    #[test]
    fn an_empty_pot_has_no_plant() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        let pot = pot_entity(&mut app, 0);

        assert!(crop_child(&mut app, pot).is_none());
    }

    #[test]
    fn the_plant_follows_every_growth_stage() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        let pot = pot_entity(&mut app, 0);
        app.world_mut()
            .get_mut::<Pot>(pot)
            .unwrap()
            .plant(CropType::CropB);
        app.update();
        let child = crop_child(&mut app, pot).unwrap();

        let expected = [
            CropStage::Growing,
            CropStage::GrowingLarge,
            CropStage::Grown,
        ];
        for stage in expected {
            {
                let mut planted = app.world_mut().get_mut::<Pot>(pot).unwrap();
                planted.water();
                planted.advance_day();
            }
            app.update();
            assert_eq!(
                app.world().get::<CropSprite>(child).unwrap().stage,
                stage,
                "the plant should show its next stage as it matures"
            );
        }
    }

    #[test]
    fn harvesting_a_pot_clears_its_plant() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        let pot = grow_pot_to_ready(&mut app, 0);
        app.update();
        assert!(crop_child(&mut app, pot).is_some());

        app.world_mut().get_mut::<Pot>(pot).unwrap().harvest();
        app.update();

        assert!(
            crop_child(&mut app, pot).is_none(),
            "harvesting must remove the plant"
        );
    }

    #[test]
    fn a_plant_comes_back_after_a_boss_fight_round_trip() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        let pot = pot_entity(&mut app, 0);
        app.world_mut()
            .get_mut::<Pot>(pot)
            .unwrap()
            .plant(CropType::CropA);
        app.update();

        request_level(&mut app, LevelId::ArenaA);
        request_level(&mut app, LevelId::Farm);

        let restored = pot_entity(&mut app, 0);
        let child = crop_child(&mut app, restored).expect("the plant is restored");
        assert_eq!(
            app.world().get::<CropSprite>(child).unwrap().crop,
            CropType::CropA
        );
    }

    #[test]
    fn the_plant_sits_above_the_soil_layer() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        let pot = pot_entity(&mut app, 0);
        app.world_mut()
            .get_mut::<Pot>(pot)
            .unwrap()
            .plant(CropType::Starter);
        app.update();

        let soil_z = app.world().get::<Transform>(pot).unwrap().translation.z;
        let child = crop_child(&mut app, pot).unwrap();
        let plant_z = app.world().get::<Transform>(child).unwrap().translation.z;
        assert!(
            plant_z > soil_z,
            "the plant draws on top of the soil: {plant_z} vs {soil_z}"
        );
    }

    #[test]
    fn a_crop_is_drawn_smaller_the_more_its_art_fills_the_frame() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);

        for (index, crop) in [(0usize, CropType::Starter), (1, CropType::CropA)] {
            let pot = pot_entity(&mut app, index);
            app.world_mut().get_mut::<Pot>(pot).unwrap().plant(crop);
            app.update();

            let child = crop_child(&mut app, pot).unwrap();
            let transform = *app.world().get::<Transform>(child).unwrap();
            let scale = crop.sprite_scale();
            assert_eq!(
                transform.scale,
                Vec3::splat(scale),
                "{crop:?} should be drawn at its own scale"
            );
            assert_eq!(
                transform.translation.y,
                CROP_VERTICAL_OFFSET * scale,
                "{crop:?} is lifted in proportion to its size so its soil lines up"
            );
        }
    }

    #[test]
    fn a_pot_phase_is_always_a_valid_frame() {
        for index in 0..64 {
            assert!(crop_phase(index) < CROP_FRAME_COUNT);
        }
    }

    #[test]
    fn plants_of_the_same_crop_do_not_all_share_a_phase() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        for index in 0..4 {
            let pot = pot_entity(&mut app, index);
            app.world_mut()
                .get_mut::<Pot>(pot)
                .unwrap()
                .plant(CropType::Starter);
        }
        app.update();

        let phases: Vec<usize> = (0..4)
            .map(|index| {
                let pot = pot_entity(&mut app, index);
                let child = crop_child(&mut app, pot).unwrap();
                app.world().get::<CropSprite>(child).unwrap().phase
            })
            .collect();
        assert!(
            phases.iter().any(|phase| *phase != phases[0]),
            "some plants must start on different frames: {phases:?}"
        );
    }

    #[test]
    fn a_watered_crop_is_marked_soaked() {
        let mut app = setup_farm_app();
        enter_playing(&mut app);
        let pot = pot_entity(&mut app, 0);
        app.world_mut()
            .get_mut::<Pot>(pot)
            .unwrap()
            .plant(CropType::CropA);
        app.update();

        let child = crop_child(&mut app, pot).unwrap();
        assert!(
            app.world().get::<CropSprite>(child).unwrap().watered,
            "planting waters the crop, so it starts soaked"
        );

        // Next day it is planted but no longer watered until the player acts.
        app.world_mut().get_mut::<Pot>(pot).unwrap().advance_day();
        app.update();
        assert!(
            !app.world().get::<CropSprite>(child).unwrap().watered,
            "the soaked tint clears once the day turns over"
        );
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
            crop_quad: None,
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
            crop_quad: None,
            dry: Some(dry.clone()),
            wet: None,
        };

        assert_eq!(mound_texture(&assets, PotState::Watered, &dry), dry);
    }
}
