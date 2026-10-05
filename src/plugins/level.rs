use crate::components::collider::Collider;
use crate::components::player::Player;
use crate::levels::grid::SolidGrid;
use crate::levels::{LevelId, PropKind, TileKind};
use crate::plugins::farm::spawn_pot;
use crate::plugins::gear::spawn_crafting_station;
use crate::plugins::interaction::{BossArenaEntry, HIGHLIGHT_Z, HighlightMarker, Interactable};
use crate::resources::level::{
    ActiveLevel, BossSpawn, LevelEntity, LevelRequest, LevelSet, PlayerSpawn, build_level,
    prop_position, prop_positions,
};
use crate::states::GameState;
use bevy::asset::RenderAssetUsages;
use bevy::ecs::system::SystemParam;
use bevy::image::{Image, ImageSampler};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::sprite_render::{TileData, TilemapChunk, TilemapChunkMeshCache, TilemapChunkTileData};

pub const ARENA_GATE_SIZE: f32 = 64.0;

/// World sprites and their highlight children sit at z = 0 and z = -0.1, so the
/// room's tiles have to be well behind both or they cover the highlights.
pub const FLOOR_Z: f32 = -10.0;

const TILE_ART_SIZE: u32 = 32;
const MIN_TILE_LAYERS: usize = 2;

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveLevel>()
            .init_resource::<SolidGrid>()
            .init_resource::<PlayerSpawn>()
            .init_resource::<BossSpawn>()
            .init_resource::<LevelRequest>()
            .init_resource::<BuiltTiles>()
            .configure_sets(Update, (LevelSet::Load, LevelSet::TileChunk).chain())
            .add_systems(
                OnEnter(GameState::Playing),
                enter_level.in_set(LevelSet::Load),
            )
            .add_systems(OnExit(GameState::Playing), despawn_all_levels)
            .add_systems(Update, apply_level_request.in_set(LevelSet::Load))
            .add_systems(Update, sync_tilemap_chunk.in_set(LevelSet::TileChunk));
    }
}

/// Loads the farm when play starts.
fn enter_level(
    mut commands: Commands,
    mut level: LevelState,
    existing: Query<(Entity, &LevelEntity)>,
    movers: Movers,
) {
    swap_level(&mut commands, LevelId::Farm, &mut level, &existing, movers);
}

/// Moves to whatever room was requested, if it is not the one already loaded.
pub fn apply_level_request(
    mut commands: Commands,
    mut request: ResMut<LevelRequest>,
    mut level: LevelState,
    existing: Query<(Entity, &LevelEntity)>,
    movers: Movers,
) {
    let Some(requested) = request.0.take() else {
        return;
    };
    if requested == level.active.id {
        return;
    }
    swap_level(&mut commands, requested, &mut level, &existing, movers);
}

/// Asks the game to show a different room. Takes effect on the next update.
pub fn request_level(mut request: ResMut<LevelRequest>, id: LevelId) {
    request.0 = Some(id);
}

fn swap_level(
    commands: &mut Commands,
    id: LevelId,
    level: &mut LevelState,
    existing: &Query<(Entity, &LevelEntity)>,
    mut movers: Movers,
) {
    let (def, grid) = build_level(id);

    for (entity, other) in existing.iter() {
        if other.level != id {
            commands.entity(entity).despawn();
        }
    }

    let pots = prop_positions(&grid, &def, PropKind::Pot);
    let station = prop_position(&grid, &def, PropKind::CraftingStation);
    let gate = prop_position(&grid, &def, PropKind::ArenaGate);
    let player_start = prop_position(&grid, &def, PropKind::PlayerSpawn).unwrap_or(Vec2::ZERO);
    let boss_start = prop_position(&grid, &def, PropKind::BossSpawn).unwrap_or(Vec2::ZERO);

    *level.active = ActiveLevel {
        id,
        def: def.clone(),
    };
    *level.player_spawn = PlayerSpawn {
        position: player_start,
    };
    *level.boss_spawn = BossSpawn {
        position: boss_start,
    };
    movers.teleport(player_start);
    *level.grid = grid;

    let tag = LevelEntity::of(id);
    let mut spawned: Vec<Entity> = Vec::new();

    for (index, position) in pots.into_iter().enumerate() {
        spawned.push(spawn_pot(commands, index, position));
    }
    if let Some(position) = station {
        spawned.push(spawn_crafting_station(commands, position));
    }
    if let Some(position) = gate {
        spawned.push(spawn_arena_gate(commands, position));
    }

    for entity in spawned {
        commands.entity(entity).insert(tag);
    }
    commands.spawn((tag, Name::new(format!("Level {}", id.file_name()))));
}

/// Everything a room change writes, gathered so the systems stay short.
#[derive(SystemParam)]
pub struct LevelState<'w> {
    active: ResMut<'w, ActiveLevel>,
    grid: ResMut<'w, SolidGrid>,
    player_spawn: ResMut<'w, PlayerSpawn>,
    boss_spawn: ResMut<'w, BossSpawn>,
}

/// The player and camera, which a room change has to move to the new spawn.
#[derive(SystemParam)]
pub struct Movers<'w, 's> {
    player: Query<'w, 's, &'static mut Transform, (With<Player>, Without<Camera2d>)>,
    camera: Query<'w, 's, &'static mut Transform, (With<Camera2d>, Without<Player>)>,
}

impl Movers<'_, '_> {
    fn teleport(&mut self, position: Vec2) {
        for mut transform in self.player.iter_mut() {
            transform.translation = position.extend(transform.translation.z);
        }
        for mut transform in self.camera.iter_mut() {
            transform.translation.x = position.x;
            transform.translation.y = position.y;
        }
    }
}

fn spawn_arena_gate(commands: &mut Commands, position: Vec2) -> Entity {
    commands
        .spawn((
            Interactable::new(),
            BossArenaEntry,
            Collider {
                size: Vec2::splat(ARENA_GATE_SIZE),
                is_solid: true,
            },
            Sprite {
                color: Color::srgb(0.55, 0.25, 0.75),
                custom_size: Some(Vec2::splat(ARENA_GATE_SIZE)),
                ..default()
            },
            Transform::from_xyz(position.x, position.y, 0.0),
            Name::new("Arena Gate"),
        ))
        .with_children(|parent| {
            parent.spawn((
                HighlightMarker,
                Sprite {
                    color: Color::srgba(1.0, 1.0, 0.0, 0.5),
                    custom_size: Some(Vec2::splat(ARENA_GATE_SIZE * 1.15)),
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, HIGHLIGHT_Z),
                Visibility::Hidden,
                Name::new("Highlight"),
            ));
        })
        .id()
}

/// Which room the visible tiles belong to, so a rebuild only happens on a change.
#[derive(Resource, Reflect, Clone, Copy, Debug, Default)]
pub struct BuiltTiles {
    pub level: Option<LevelId>,
}

/// Draws the room's tiles as a single chunk. Needs the render world, so it does
/// nothing in headless tests that run without a renderer.
fn sync_tilemap_chunk(
    mut commands: Commands,
    active: Res<ActiveLevel>,
    grid: Res<SolidGrid>,
    mut built: ResMut<BuiltTiles>,
    cache: Option<Res<TilemapChunkMeshCache>>,
    mut images: Option<ResMut<Assets<Image>>>,
    existing: Query<Entity, With<TileChunkMarker>>,
) {
    if cache.is_none() || images.is_none() {
        return;
    }
    if built.level == Some(active.id) && existing.iter().next().is_some() {
        return;
    }

    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

    let def = &active.def;
    let tile_data = level_tile_data(def);
    let center = grid.center();

    commands.spawn((
        TileChunkMarker,
        TilemapChunk {
            chunk_size: UVec2::new(def.width, def.height),
            tile_display_size: UVec2::splat(grid.tile_size() as u32),
            tileset: placeholder_tileset(images.as_mut().unwrap()),
            ..default()
        },
        TilemapChunkTileData(tile_data),
        Transform::from_xyz(center.x, center.y, FLOOR_Z),
        Name::new("Level Tiles"),
    ));
    built.level = Some(active.id);
}

#[derive(Component)]
struct TileChunkMarker;

pub fn despawn_all_levels(mut commands: Commands, existing: Query<(Entity, &LevelEntity)>) {
    for (entity, _) in existing.iter() {
        commands.entity(entity).despawn();
    }
}

/// Flattens a level into the tile data a [`TilemapChunk`] renders.
///
/// No row flipping happens here: `TilemapChunkTileData` is indexed top-down,
/// exactly like the level file. Only world-space lookups need the flip, which
/// is why [`SolidGrid`] and the spawn points use bottom-up rows instead.
pub fn level_tile_data(def: &crate::utils::level_parse::LevelDef) -> Vec<Option<TileData>> {
    def.tiles
        .iter()
        .map(|kind| Some(TileData::from_tileset_index(kind.tileset_index())))
        .collect()
}

/// Builds a flat-colour tileset so rooms are visible before the art lands.
///
/// Bevy samples the tileset as a 2D array with one layer per `TileKind`, so the
/// layers must line up with `TileKind::tileset_index`.
fn placeholder_tileset(images: &mut Assets<Image>) -> Handle<Image> {
    let colors: Vec<[u8; 4]> = TileKind::ALL
        .iter()
        .filter(|kind| **kind != TileKind::Void)
        .map(|kind| placeholder_color(*kind))
        .collect();
    placeholder_tileset_from_colors(images, &colors)
}

fn placeholder_color(kind: TileKind) -> [u8; 4] {
    match kind {
        TileKind::Void => [0, 0, 0, 0],
        TileKind::Grass => [86, 140, 74, 255],
        TileKind::Dirt => [140, 100, 68, 255],
        TileKind::Water => [70, 110, 180, 255],
        TileKind::Wall => [48, 48, 58, 255],
    }
}

fn placeholder_tileset_from_colors(
    images: &mut Assets<Image>,
    colors: &[[u8; 4]],
) -> Handle<Image> {
    let layers = colors.len().max(MIN_TILE_LAYERS);
    let pixels = (TILE_ART_SIZE * TILE_ART_SIZE) as usize;
    let mut data = Vec::with_capacity(pixels * layers * 4);
    for index in 0..layers {
        let color = colors.get(index).copied().unwrap_or([0, 0, 0, 0]);
        for _ in 0..pixels {
            data.extend_from_slice(&color);
        }
    }

    let mut image = Image::new(
        Extent3d {
            width: TILE_ART_SIZE,
            height: TILE_ART_SIZE,
            depth_or_array_layers: layers as u32,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    images.add(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_placeholder_tileset_has_a_layer_per_drawn_tile() {
        let mut images = Assets::<Image>::default();
        let handle = placeholder_tileset(&mut images);
        let image = images.get(&handle).unwrap();

        assert_eq!(
            image.texture_descriptor.size.depth_or_array_layers, 4,
            "grass, dirt, water, wall"
        );
        assert_eq!(
            image.data.as_ref().unwrap().len(),
            (TILE_ART_SIZE * TILE_ART_SIZE * 4 * 4) as usize,
            "one RGBA texel per pixel per layer"
        );
        assert_eq!(image.width(), TILE_ART_SIZE);
        assert_eq!(image.height(), TILE_ART_SIZE);
    }

    #[test]
    fn the_placeholder_layers_match_the_tileset_indices() {
        let mut images = Assets::<Image>::default();
        let handle = placeholder_tileset(&mut images);
        let image = images.get(&handle).unwrap();
        let data = image.data.as_ref().unwrap();
        let layer_size = (TILE_ART_SIZE * TILE_ART_SIZE * 4) as usize;

        for kind in TileKind::ALL.iter().filter(|k| **k != TileKind::Void) {
            let index = kind.tileset_index() as usize;
            let pixel = &data[index * layer_size..index * layer_size + 4];
            assert_eq!(
                pixel,
                placeholder_color(*kind),
                "layer {index} should be {kind}"
            );
        }
    }

    #[test]
    fn a_tileset_always_has_at_least_two_layers() {
        let mut images = Assets::<Image>::default();
        let handle = placeholder_tileset_from_colors(&mut images, &[[1, 2, 3, 4]]);
        let image = images.get(&handle).unwrap();
        assert_eq!(image.texture_descriptor.size.depth_or_array_layers, 2);
        assert_eq!(
            image.data.as_ref().unwrap().len(),
            (TILE_ART_SIZE * TILE_ART_SIZE * 4 * 2) as usize,
            "padding is filled, not left short"
        );
    }

    #[test]
    fn tile_data_keeps_the_file_row_order() {
        let def = crate::utils::level_parse::parse("[tiles]\n#\n.\n").unwrap();
        let data = level_tile_data(&def);
        assert_eq!(data.len(), 2);
        assert_eq!(
            data[0].unwrap().tileset_index,
            TileKind::Wall.tileset_index()
        );
        assert_eq!(
            data[1].unwrap().tileset_index,
            TileKind::Dirt.tileset_index()
        );
    }

    #[test]
    fn tile_data_marks_void_with_the_discard_index() {
        let def = crate::utils::level_parse::parse("[tiles]\n# #\n").unwrap();
        let data = level_tile_data(&def);
        assert_eq!(data[1].unwrap().tileset_index, 0xffff);
    }

    #[test]
    fn tile_data_has_one_entry_per_tile() {
        for id in LevelId::ALL {
            let def = id.parse().unwrap();
            let data = level_tile_data(&def);
            assert_eq!(data.len(), (def.width * def.height) as usize);
            assert!(data.iter().all(Option::is_some));
        }
    }

    #[test]
    fn void_is_the_only_transparent_placeholder() {
        assert_eq!(placeholder_color(TileKind::Void)[3], 0);
        for kind in TileKind::ALL.iter().filter(|k| **k != TileKind::Void) {
            assert_eq!(placeholder_color(*kind)[3], 255);
        }
    }
}
