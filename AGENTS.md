# Plan - dl-jam (Dreamlayer Game Jam)

## 1. Project Overview
- **Genre**: Farming sim + Boss rush (roguelite day cycle)
- **Core Loop**: Day (farm 9 pots → craft gear) → Night (attempt boss) → Win/Die → Next Day
- **Progression**: 3 crops (1 starter, unlock 2nd after Boss A, 3rd after Boss B) → 4 gear sets → Dual boss finale
- **Win Condition**: Beat Boss A + Boss B once → Unlock & beat Dual Boss (both at once)
- **Tech**: Bevy 0.19.1, wasm-bindgen, Rust 2024 edition
- **Target**: Desktop WASM on itch.io (HTML5)
- **Assets**: Manual Dreamlayer (art) → `./assets/` + Free audio libraries
- **Solo dev**

## 2. Architecture

### App States (`bevy_state`)
```
GameState::LoadingAssets            (boot: preload the farm behind a bar)
    → GameState::MainMenu            (title screen: New Game; difficulty/load reserved)
    → GameState::Playing { day: u32, phase: DayPhase }
        → DayPhase::Farming         (plant, water, harvest, craft)
        → DayPhase::BossSelect      (choose Boss A / B / Dual)
        → DayPhase::BossFight       (combat)
        → DayPhase::Loading         (preload the next scene; stays in Playing)
        → DayPhase::Result          (victory/defeat screen → next day)
    → GameState::Victory            (dual boss beaten; Continue Playing / Main Menu)
```

Pause is **not** a state: `PauseMenu` is a resource overlay that sits on top of
any `Playing` phase and freezes the world by pausing `Time<Virtual>`.

### Plugins (modular)
| Plugin | Responsibility |
|--------|----------------|
| `FarmPlugin` | Pot grid, crop growth (3 stages), water/harvest, crop unlocks |
| `GearPlugin` | Crafting UI, 4 gear sets (weapon + armor), material requirements |
| `BossPlugin` | 2 bosses + dual boss, attack patterns, material drops |
| `DayCyclePlugin` | Day/Night transitions, progression tracking |
| `PersistencePlugin` | Save/Load architecture (stubbed early, implement late) |
| `LevelPlugin` | Loads rooms from `levels/*.txt`, spawns tiles + props, owns `SolidGrid` |
| `MainMenuPlugin` | Title screen: `New Game` (resets the run), reserved difficulty/load slots |
| `PausePlugin` | Pause overlay: Resume, Return to Main Menu, reserved difficulty slot |
| `UIPlugin` | Forge, inventory, boss select and planting panels (presentation only) |
| `ui_theme` | Shared menu widgets: skinnable buttons, panes, animated previews |
| `HudPlugin` | Player/boss health bars, day counter, phase input hints |
| `AudioPlugin` | Music/SFX management |

### Menu system
- Every menu button is a `ui_theme::MenuButton` styled from `MenuButtonVisuals`.
  It resolves to an explicit `ButtonArt` path, then a per-button
  `images/ui/buttons/<skin>_{normal,hovered,selected}.png` if that file exists,
  then the shared `frame_*.png`. `build.rs` bakes the list of files that exist
  into the binary, so a button with no custom art falls back quietly instead of
  logging a missing path (same on native and wasm). **Adding art requires a
  rebuild** (`cargo run` / `trunk`). The shared frame is a 9-slice with a 14px
  inset and a 44px row height; author override art at the same inset.
- Button skins: `new_game`, `difficulty`, `load_game`, `resume`,
  `return_to_main_menu`, `continue_playing`, `boss_a`, `boss_b`, `dual_boss`,
  `infinite_mode`, `starter_crop`, `crop_a`, `crop_b`. Gear and inventory rows
  intentionally share the frame (their labels are dynamic).
- Menus spawn/despawn from resource flags (`open`/`selected`) in their
  `systems/*` module; `plugins/ui.rs` only lays out and refreshes the visuals.
- Selection is a `MenuButtonSelected` flag; a single `style_menu_buttons` system
  in `MenuStyleSet` paints background (hover) and border (selection). Refresh
  systems run `.before(MenuStyleSet)` so a change lands the same frame.
- Animated previews (`AnimatedPreview`) reuse the 5×5 player atlas in a UI
  `ImageNode`; the forge alternates light/heavy swings for weapons and idles
  armor, the planting pane shows the sapling placeholder. Boss select shows the
  matching concept art (both halves for the dual boss).
- Reserved slots (difficulty, load game, infinite mode) are spawned hidden so
  they keep their layout room and can be revealed via `MenuFeatureFlags`.

### Resources (all `#[derive(Resource, Serialize, Deserialize, Reflect)]`)
```rust
#[derive(Resource, Serialize, Deserialize, Reflect, Default)]
struct RunData {
    day: u32,
    gear: PlayerGear,
    inventory: Inventory,
    crops_unlocked: CropUnlocks,
    bosses_beaten: BossProgress,
    stats: RunStats,
}

#[derive(Serialize, Deserialize, Reflect, Default)]
struct PlayerGear {
    weapon: Option<GearPiece>,  // equipped weapon
    armor: Option<GearPiece>,   // equipped armor
}

#[derive(Serialize, Deserialize, Reflect, Default)]
struct Inventory {
    crops: HashMap<CropType, u32>,      // harvested crops
    materials: HashMap<MaterialType, u32>, // boss drops
}

#[derive(Serialize, Deserialize, Reflect, Default)]
struct CropUnlocks {
    starter: bool,      // always true
    boss_a: bool,       // unlocked after beating Boss A
    boss_b: bool,       // unlocked after beating Boss B
}

#[derive(Serialize, Deserialize, Reflect, Default)]
struct BossProgress {
    boss_a: bool,
    boss_b: bool,
    dual_boss_unlocked: bool,
    dual_boss_beaten: bool,
}

#[derive(Resource, Serialize, Deserialize, Reflect, Default)]
struct FarmState {
    pots: Vec<Pot>,  // pot state kept across room swaps so crops survive boss fights
}

#[derive(Resource, Reflect, Default)]
struct CropSelectMenu {
    open: bool,
    selected: usize,     // index into CropType::ALL
    pending_pot: usize,  // Pot::index the picker was opened for
}
```

### Components (all `#[derive(Component, Reflect, Serialize, Deserialize)]`)
- `Player` + `Health` + `Movement`
- `Pot { index: usize, state: PotState }` — 9 pots fixed grid
- `Crop { crop_type: CropType, stage: u8, grow_timer: Timer }`
- `GearPiece { set: GearSet, slot: GearSlot, weapon_type: WeaponType }`
- `Boss { id: BossId, phase: u8, max_hp: f32 }`
- `AttackPattern { timer: Timer, pattern: PatternType }`
- `MaterialDrop { material: MaterialType }`
- `CropOption { crop: CropType, index: usize }` — clickable row in the crop picker

### Events
- `CropPlanted(CropType)`, `CropWatered`, `CropHarvested(CropType)`
- `GearCrafted(GearPiece)`, `GearEquipped(GearSlot)`
- `BossPhaseChanged(BossId, u8)`, `BossDefeated(BossId)`, `PlayerDied`
- `DayAdvanced(u32)`, `CropUnlocked(CropType)`, `DualBossUnlocked`
- `SaveRequested`, `LoadRequested`

## 3. Gear System (Per-Piece Crafting)

Each piece is crafted individually (8 recipes); a piece is owned once and
auto-equips when crafted.

| Piece | Cost |
|-------|------|
| Starter Spearblade | Starter Crop ×2 |
| Starter Armor | Starter Crop ×3 |
| Boss A Spearblade | Crop A ×2 + Boss A Material 1 ×1 |
| Boss A Armor | Crop A ×3 + Boss A Material 2 ×2 |
| Boss B Spearblade | Crop B ×2 + Boss B Material 1 ×1 |
| Boss B Armor | Crop B ×3 + Boss B Material 2 ×2 |
| Master Spearblade | each Crop ×1 + both bosses' Material 1 ×2 |
| Master Armor | each Crop ×3 + both bosses' Material 2 ×4 |

- 4 sets, each 1 weapon (affects attack pattern/damage) + 1 armor (affects HP/defense)
- Crafting UI shows one recipe row per piece; owned pieces get an equip row
- No currency — pure material gating

## 4. Boss Design

| Boss | Access           | Patterns | Drops |
|------|------------------|----------|-------|
| **Boss A** | Imediate         | 3 patterns, 2 phases | Boss A Material 1 ×0-2 + Boss A Material 2 ×1-3 (rolled), unlocks Boss A Crop |
| **Boss B** | Imediate         | 3 patterns, 2 phases | Boss B Material 1 ×0-2 + Boss B Material 2 ×1-3 (rolled), unlocks Boss B Crop |
| **Dual Boss** | After A+B beaten | Combined patterns, shared HP | Victory |

## 5. Development Workflow

### System Conventions
- **State checks go through `Phase`** (`src/states.rs`), a `SystemParam` bundling
  `State<GameState>` + `State<DayPhase>`. Use `phase.is_playing()`,
  `phase.is_farming()` or `phase.blocks_world()` instead of matching on the two
  states separately. `blocks_world` is the "panel may react to input" check.
- **Bundle repeated params.** Systems that shared 7+ `Query`/`Res` params were
  split into `SystemParam` structs (`InteractableLookups`, `CraftingWork`,
  `EquipWork`, `LevelState`). `cargo clippy` is warning-free; keep it that way.
- **Logging uses `log::` macros**, not `eprintln!`. `log` is already a
  dependency (`max_level_debug`), so debug output disappears in release builds.

### Commands
```bash
# Native dev (primary iteration) - runs src/main.rs
cargo run

# WASM test build (frequent) - uses src/lib.rs entry point
trunk serve --open

# Code quality
cargo check && cargo clippy && cargo fmt --check
```

### Entry Points
- **main.rs** (native): Minimal, creates `App` and adds `GamePlugin` from `default.rs`
- **lib.rs** (WASM): Minimal, exports `GamePlugin` from `default.rs` for `wasm-bindgen`
- **default.rs**: Main game plugin (`GamePlugin`) — all plugin registration, systems, resources

## 6. Asset Pipeline

### Directory Structure
```
assets/
├── images/
│   ├── crops/        # 3 types × 3 stages = 9 sprites
│   ├── gear/         # 4 sets × (weapon + armor) = 8 sprites
│   ├── bosses/       # Boss A, Boss B, Dual (spritesheets)
│   ├── ui/           # panels, buttons, icons, pot frames
│   │   └── buttons/  # frame_normal/hovered/selected.png (9-slice, shared)
│   └── tiles/        # floor, walls, arena
├── audio/
│   ├── sfx/          # .ogg (plant, water, harvest, craft, hit, boss)
│   └── music/        # .ogg (farm_day, boss_night, victory)
└── fonts/
    └── .ttf
```

### Loading
- Both entry points set `AssetPlugin { meta_check: AssetMetaCheck::Never, .. }`.
  Web hosts answer a missing `<asset>.meta` with a 200 HTML page (trunk's SPA
  fallback), which Bevy would otherwise feed to the RON parser and fail the load.
  `load_folder` is unusable on WASM (`read_directory` is unsupported), so sheets
  are requested by explicit path.
- `LoadingPlugin` (`src/plugins/loading.rs`) owns the progress bar. Boot uses
  `GameState::LoadingAssets`; in-game farm↔arena swaps use `DayPhase::Loading`
  so `Playing` (and the run state under it) is never left.
- `resources/scene_assets.rs` maps a `LoadTarget` to the exact sheets a scene
  needs. `SceneAssetManifest` keeps strong handles for the current scene, reusing
  shared ones and dropping the rest, so the scene being left unloads.
- Preload scope is the worn player look plus the current boss: farm and
  boss-select hold the farmer (boss-select also grabs one idle frame per boss for
  the menu), a single arena holds the armor look + its boss, the dual arena holds
  both bosses. Each sheet decodes to ~6.5 MB (1280×1280 RGBA), so this matters:
  farm ~0.37 GB, single arena ~0.58 GB, dual ~0.89 GB.
- `TextureAtlasLayout` for animated sprites (crops, bosses); the layout is shared
  across every sheet and is never unloaded.
- Audio: OGG Vorbis, mono SFX, stereo music, <2MB total
- Room tiles are **not** loaded from `assets/`; they come from `levels/*.txt`
  (see §11b) and render through `TilemapChunk`
- `/assets` is gitignored, so trunk's `<link data-trunk rel="copy-dir" href="assets"/>`
  in `index.html` copies it into `dist/`.

## 7. Input Scheme (Desktop WASM)

| Action | Key/Mouse |
|--------|-----------|
| Move | WASD / Arrow Keys |
| Interact (water/harvest/craft/attack) | Left Click / Space |
| Plant (on an empty pot) | Opens the crop picker: Arrows/WASD select, Enter/E/Space or click plants, Esc cancels |
| Gear Swap | 1 (weapon), 2 (armor) / Scroll |
| Open Crafting | Tab / C |
| Open Inventory | I (toggle, works in every phase) |
| Boss Select | Click UI |
| Pause | Escape (opens the pause overlay; Resume is auto-selected) |

Panels are mutually exclusive and freeze movement + world interaction while open.
Esc closes whichever panel is open; with none open it opens the pause overlay
(`PausePlugin` claims Esc before the panel handlers so one press never both
closes a panel and opens pause). Interacting with an empty pot opens the
`CropSelectMenu` picker (locked crops are shown dimmed with their unlock hint
and their harvest yield); the crop is planted only when the player confirms.

## 8. Persistence Architecture (Save/Load Ready)

### SaveManager Trait
```rust
trait SaveBackend {
    fn save(&self, key: &str, data: &[u8]) -> Result<(), SaveError>;
    fn load(&self, key: &str) -> Result<Vec<u8>, SaveError>;
    fn exists(&self, key: &str) -> bool;
}
```

### Backends
- **Native**: `FileBackend` → `bincode` to `save.dat`
- **WASM**: `IndexedDBBackend` → `js_sys::Object` via `wasm-bindgen` + `web-sys`

### Integration
- `SaveManager` resource with active backend
- Auto-save on: `DayAdvanced`, `BossDefeated`, `PlayerDied`, `GearCrafted`
- Load check in `LoadingAssets` → show "Continue" on main menu if save exists
- Stub `SaveManager` early, implement backends late

## 9. Performance (WASM)
- Texture atlases (max 2048×2048, pack crops/gear/UI separately)
- `FixedTimestep(60Hz)` for game logic
- `Query` filters: prefer `With<>`/`Without<>` over `Changed<>`
- Profile: `cargo build --profile wasm-release` + `wasm-opt -Oz`
- Target: <5MB WASM, <10MB total assets

## 10. Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Boss patterns too complex | Start with 1 pattern per boss, add incrementally |
| Dreamlayer assets delayed | Kenney.nl placeholders in `assets/placeholder/` |
| WASM bugs late | Frequent WASM testing from start |
| Scope creep | Hard cap: 3 crops, 4 gear sets, 2+1 bosses |
| Save/Load time | Stub early, implement late only if time |
| Dual boss balance | Test with max gear (Master set) as baseline |

## 11. File Structure (src/)
```
src/
├── main.rs                 # Native App entry
├── lib.rs                  # WASM entry
├── default.rs              # Main game plugin (GamePlugin) — all plugins, systems, resources
├── states.rs               # GameState, DayPhase + Phase (bundled state param)
├── levels/
│   ├── mod.rs              # LevelId + include_str! of levels/*.txt
│   ├── legend.rs           # TileKind, PropKind (chars, solidity, tileset layers)
│   └── grid.rs             # SolidGrid — solidity lookup, tile↔world, collision
├── plugins/
│   ├── farm.rs
│   ├── gear.rs
│   ├── level.rs            # LevelPlugin — load rooms, spawn TilemapChunk + props
│   ├── loading.rs          # LoadingPlugin — scene preload + progress bar
│   ├── main_menu.rs        # MainMenuPlugin — title screen + New Game reset
│   ├── pause.rs            # PausePlugin — overlay, virtual-time pause
│   ├── boss.rs
│   ├── day_cycle.rs
│   ├── persistence.rs
│   ├── ui.rs               # forge / inventory / boss select / planting panels
│   ├── ui_theme.rs         # shared skinnable buttons, panes, animated previews
│   ├── hud.rs              # HudPlugin — player/boss health bars, day counter, input hints
│   └── audio.rs
├── components/
│   ├── player.rs
│   ├── pot.rs
│   ├── crop.rs
│   ├── gear.rs
│   └── boss.rs
├── resources/
│   ├── run_data.rs
│   ├── inventory.rs
│   ├── crop_select.rs      # CropSelectMenu — crop picker state + target pot
│   ├── crafting_menu.rs    # CraftingMenu — forge state + notice
│   ├── boss_select.rs      # BossSelectMenu — boss list + confirmation
│   ├── inventory_panel.rs  # InventoryPanel — inventory state + notice
│   ├── main_menu.rs        # MainMenu, MenuFeatureFlags, NewGameRequested
│   ├── pause.rs            # PauseMenu — pause overlay state
│   ├── menu_text.rs        # placeholder gear descriptions, crop yields, boss drops
│   ├── scene_assets.rs     # LoadTarget + SceneAssetManifest + LoadingContext
│   ├── level.rs            # ActiveLevel, LevelEntity, PlayerSpawn, BossSpawn, LevelRequest
│   └── save_manager.rs
├── events.rs
├── systems/
│   ├── farming.rs
│   ├── crafting.rs
│   ├── crop_select.rs      # CropSelectPlugin — picker input + planting
│   ├── combat.rs
│   ├── day_cycle.rs
│   └── level_movement.rs   # Player movement against SolidGrid + solid entities
└── utils/
    ├── save_backend.rs
    ├── interaction_math.rs
    └── level_parse.rs      # ASCII level file → LevelDef (pure, no Bevy types)
```

---

## 11b. Levels

Rooms are ASCII text files in `levels/`, compiled into the binary with
`include_str!` — no runtime file loading, so levels work in tests unchanged.

**Format reference: [`levels/README.md`](levels/README.md)** — read it before
editing or adding a level.

- `levels/farm.txt`, `levels/arena_a.txt`, `levels/arena_b.txt`, `levels/arena_dual.txt`
- `LevelPlugin` loads a room on entering `Playing` and on `LevelRequest`, spawns the
  `TilemapChunk` plus props, and tags them `LevelEntity` so room swaps despawn cleanly.
- One `[tiles]` grid + one optional `[props]` list. Tile chars: `,` grass, `.` dirt,
  `~` water, `#` wall, space void. Prop markers: `^ p s x b` with `x,y` coords.
- Grid rows are **top-down** (row 0 = first line); world space is **y-up**.
  `LevelDef::world_row` converts; `SolidGrid::is_solid` takes a world row.
- `#` is a wall, not a comment — comments only count outside the `[tiles]` grid.
- `SolidGrid` holds blocking tiles in world order; walls are never spawned as
  entities with colliders, which is what keeps collision cheap. Movement resolves
  one axis at a time and sub-steps, so corners slide and nothing tunnels.
- Bevy 0.19 removed the old tilemap. Tiles use `TilemapChunk` (one draw call) from
  `bevy::sprite_render`, whose tile data is indexed in **world order** (row 0 =
  bottom), like `calculate_tile_transform` and `SolidGrid`. `level_tile_data`
  flips the file's top-down rows into that order once when building the chunk.
- `cargo test levels` validates every level file (sealed border, one player
  spawn, no props inside walls), so a broken level fails the build.

---

## 12. Git Policy
- **Never** run git commands requiring SSH or GPG (push, commit, tag, etc.)
- `git commit` is configured to always require signing — do not attempt commits
- `git add` should also not be used
- Only safe commands: `git status`, `git diff`, `git log`

---

## 13. Unit Testing Guidelines

### Test Organization
- **Unit tests**: `#[cfg(test)]` modules inside each source file (`src/**/*.rs`)
- **Integration tests**: `tests/integration/*.rs` using full `GamePlugin` (needs a
  `[[test]]` entry in `Cargo.toml` per file, plus `pub mod` on the lib)

### Constraints
- **No camera/rendering/window tests** — Bevy limitation
- Extract pure math/logic to `utils/` for testability
- Test systems via `App` + `World` with minimal resources

### Coverage Target
- **70% line coverage** for unit tests
- **50% line coverage** for integration tests
- Measure: `cargo llvm-cov --workspace --lcov`

### Commands
```bash
# Native
cargo test

# WASM
wasm-pack test --headless --chrome -- --test-threads=1

# Coverage
cargo llvm-cov --workspace --lcov --output-path lcov.info
```

### Test Categories by Priority

| Priority | Area | Examples |
|----------|------|----------|
| High | Movement/Collision | Input mapping, AABB detection, response physics |
| High | State Machine | GameState/DayPhase transitions, enter/exit systems |
| High | Farming | Pot states, growth timers, watering, harvest, unlocks |
| High | Crafting | Recipe validation, material consumption, equip logic |
| Medium | Day Cycle | Phase order, boss select filtering, dual boss unlock |
| Medium | Combat | Boss phases, attack patterns, damage, defeat drops |
| Medium | Persistence | SaveManager round-trip, auto-save triggers, backends |
| Low | Resources | Serialization, defaults, Reflect |

### Fixtures
- Inline builders per test file (no shared `tests/fixtures/`)
- Simple `setup_app()` helper returning configured `App`

> **Rule**: Tests are only implemented **after** the corresponding feature is implemented. Do not write tests for planned/unimplemented features.


## Downloading images from DreamLayer
- this is how the other agents have successfully downloaded the output images: curl -L -H "Authorization: Bearer $DREAMLAYER_API_KEY" -o file.png "https://"