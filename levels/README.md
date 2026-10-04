# Level format

Levels are plain text files in this folder. They are compiled into the binary
with `include_str!`, so there is no loading step, no async wait, and no runtime
file access - which also means the game works in tests and on the web unchanged.

| File | Room | Size |
|------|------|------|
| `farm.txt` | Daytime hub: pond, 9-pot field, crafting station, arena gate | 40x24 |
| `arena_a.txt` | Boss A: open room, four pillars | 36x20 |
| `arena_b.txt` | Boss B: central cross, four lanes | 36x20 |
| `arena_dual.txt` | Finale: corner pillars, central pool | 40x24 |

## Adding a level

1. Copy an existing file and edit the grid.
2. Add a variant to `LevelId` in `src/levels/mod.rs` with an `include_str!` arm.
3. Run `cargo test`. The suite checks every level file, so a typo in the grid or
   a prop inside a wall fails the build rather than the game.

## How a level reaches the game

`LevelPlugin` (`src/plugins/level.rs`) loads a room on entering `GameState::Playing`
and whenever something sets `LevelRequest`:

```
farm.txt ─include_str!─▶ parse() ─▶ LevelDef ─┬─▶ SolidGrid   (collision)
                                              ├─▶ level_tile_data() (TilemapChunk)
                                              └─▶ props (pots, station, gate)
```

- Every spawned entity is tagged `LevelEntity { level }`, so switching rooms
  despawns only what belonged to the room being left.
- The player and camera are teleported to the new room's `^` marker.
- Walls are never entities. `SolidGrid` answers solidity, and
  `systems/level_movement.rs` moves the player against it plus the room's solid
  entities (pots, station) in a single axis-separated pass.
- Add a `[[test]]` entry in `Cargo.toml` for any new file under
  `tests/integration/`, since cargo only auto-discovers `tests/*.rs`.

## File structure

```
# comments start with a hash, outside the tile grid
tile_size = 32

[tiles]
########################################
#,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#
#,,,,,,,,,,,,,,,,,,,,,,,,.........,,,,,#
########################################

[props]
^ 4,20
s 8,19
p 26,11
```

A file has three parts, in this order:

1. **Header** - optional `key = value` lines before the first section.
2. **`[tiles]`** - required, exactly one. One rectangle of characters.
3. **`[props]`** - optional, at most one, must come after `[tiles]`.

Anything else is a parse error with a line number, so mistakes are reported
e.g. `line 14: row is 39 tiles wide, expected 40`.

### Header

| Key | Default | Meaning |
|-----|---------|---------|
| `tile_size` | `32` | World units per tile. Also the size of one cell in the tileset art. |

`tile_size` must be a positive integer. Room size comes from the grid, not the
header.

### `[tiles]` - what the room is made of

One character per tile, rows top to bottom. Every row must be exactly as wide
as the first row (trailing spaces are ignored). A space means "void": nothing is
drawn there and nothing blocks.

| Char | Tile | Blocks movement | Tileset layer |
|------|------|-----------------|---------------|
| `,` | Grass | no | 0 |
| `.` | Dirt | no | 1 |
| `~` | Water | yes | 2 |
| `#` | Wall | yes | 3 |
| space | Void | no | none (skipped by the shader) |

Two traps worth knowing:

- **`#` is a wall, not a comment.** Comments are only recognised outside the
  `[tiles]` grid, because every wall row starts with a hash.
- **Rows are top-down.** The first row is the top wall of the room. See
  [Coordinates](#coordinates).

### `[props]` - what spawns in the room

One prop per line, as `marker x,y`. Coordinates are tile coordinates with `0,0`
at the **top-left** of the grid, matching the drawing above. Props are read in
file order, and that order is what assigns pot indices.

| Marker | Prop | Notes |
|--------|------|-------|
| `^` | Player spawn | Exactly one per level. |
| `p` | Farm pot | Order becomes the pot index, so keep them top-to-bottom, left-to-right. |
| `s` | Crafting station | At most one. |
| `x` | Arena gate | At most one. |
| `b` | Boss spawn | Exactly one per arena. |

The parser rejects a prop that is out of bounds, sits on a blocking tile
(wall or water), or shares a tile with another prop.

## Coordinates

There are two row orders in play, and mixing them up flips your level.

| Where | Row 0 is | Used by |
|-------|----------|---------|
| Level file | the **top** row | drawing, `LevelDef::tile`, `LevelDef::is_solid` |
| World space | the **bottom** row | spawning, movement, `SolidGrid` |

`LevelDef::world_row(file_row)` converts between them, and `SolidGrid` does the
conversion once when it is built. Its `is_solid(col, row)` takes a **world** row.

Rendering is the one place where no flip is needed: Bevy's `TilemapChunkTileData`
is indexed top-down, exactly like the file. Only `TilemapChunk::calculate_tile_transform`
is y-up.

## What the parser guarantees

`parse()` in `src/utils/level_parse.rs` rejects:

- unknown tile characters (reports the column), unknown section names, unknown header keys
- rows of differing widths, and an empty tile grid
- props out of bounds, on solid tiles, duplicated on one tile, or with malformed coordinates
- `[props]` before `[tiles]`, or a repeated section

`src/levels/mod.rs` additionally asserts, for every level file: exactly one
player spawn, a sealed wall border on all four sides, no prop inside a wall, and
enough open floor to move around in.

## Tiles

`TileKind::tileset_index` maps each tile to a layer of the tileset texture, and
Bevy samples that texture as a 2D array. When real art lands, export the tileset
as a uniform grid with one cell per layer in index order:

```
+-------+-------+-------+-------+
| grass | dirt  | water | wall  |
|   0   |   1   |   2   |   3   |
+-------+-------+-------+-------+
```

Load it with `ImageArrayLayout::GridSize { tile_width_pixels, tile_height_pixels }`
so the cell size defines the layers. Until then the tiles are generated in code
as flat colours. Two engine details to keep in mind: the layer count comes from
`depth_or_array_layers` (a `D2` texture with depth > 1 is the array), and an
array texture needs at least two layers or wgpu rejects it.

## Collision

`SolidGrid` (`src/levels/grid.rs`) holds the blocking tiles in world order and
answers `is_solid`, `aabb_hits_solid` and `move_and_collide`. Movement resolves
one axis at a time so corners slide, and large steps are split into sub-steps so
nothing tunnels through a wall. Outside the room counts as solid.

Walls are never spawned as entities with colliders - that stays a lookup in the
grid, which is what keeps collision cheap.

## Not wired up yet

- `DayPhase::BossFight` does not switch rooms by itself. It is driven by
  `LevelRequest`, which a test or the boss flow sets:
  `app.world_mut().resource_mut::<LevelRequest>().0 = Some(LevelId::ArenaA);`
- `BossSpawn` (the `b` marker) is stored in a resource but nothing reads it yet -
  that is `BossPlugin`'s job.
- Tiles are flat placeholder colours. See [Tiles](#tiles) for the swap.
