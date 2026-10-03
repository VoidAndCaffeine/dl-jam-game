# Agents.md - dl-jam (Dreamlayer Game Jam)

## 1. Project Overview
- **Genre**: Farming sim + Boss rush (roguelite day cycle)
- **Core Loop**: Day (farm 9 pots → craft gear) → Night (attempt boss) → Win/Die → Next Day
- **Progression**: 3 crops (1 starter, unlock 2nd after Boss A, 3rd after Boss B) → 4 gear sets → Dual boss finale
- **Win Condition**: Beat Boss A + Boss B once → Unlock & beat Dual Boss (both at once)
- **Tech**: Bevy 0.19.1, wasm-bindgen, Rust 2024 edition
- **Target**: Desktop WASM on itch.io (HTML5)
- **Assets**: Manual Dreamlayer (art) → `./assets/` + Free audio libraries
- **Timeline**: 8.5 days (Oct 3 14:12 → Oct 11 21:30)
- **Solo dev**

## 2. Architecture

### App States (`bevy_state`)
```
GameState::LoadingAssets
    → GameState::Playing { day: u32, phase: DayPhase }
        → DayPhase::Farming     (plant, water, harvest, craft)
        → DayPhase::BossSelect  (choose Boss A / B / Dual)
        → DayPhase::BossFight   (combat)
        → DayPhase::Result      (victory/defeat screen → next day)
    → GameState::Victory        (dual boss beaten)
```

### Plugins (modular)
| Plugin | Responsibility |
|--------|----------------|
| `FarmPlugin` | Pot grid, crop growth (3 stages), water/harvest, crop unlocks |
| `GearPlugin` | Crafting UI, 4 gear sets (weapon + armor), material requirements |
| `BossPlugin` | 2 bosses + dual boss, attack patterns, material drops |
| `DayCyclePlugin` | Day/Night transitions, progression tracking |
| `PersistencePlugin` | Save/Load architecture (stubbed Day 4, implement Day 8-9) |
| `UIPlugin` | HUD, crafting menu, boss select, result screens |
| `AudioPlugin` | Music/SFX management |

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
```

### Components (all `#[derive(Component, Reflect, Serialize, Deserialize)]`)
- `Player` + `Health` + `Movement`
- `Pot { index: usize, state: PotState }` — 9 pots fixed grid
- `Crop { crop_type: CropType, stage: u8, grow_timer: Timer }`
- `GearPiece { set: GearSet, slot: GearSlot, weapon_type: WeaponType }`
- `Boss { id: BossId, phase: u8, max_hp: f32 }`
- `AttackPattern { timer: Timer, pattern: PatternType }`
- `MaterialDrop { material: MaterialType }`

### Events
- `CropPlanted(CropType)`, `CropWatered`, `CropHarvested(CropType)`
- `GearCrafted(GearPiece)`, `GearEquipped(GearSlot)`
- `BossPhaseChanged(BossId, u8)`, `BossDefeated(BossId)`, `PlayerDied`
- `DayAdvanced(u32)`, `CropUnlocked(CropType)`, `DualBossUnlocked`
- `SaveRequested`, `LoadRequested`

## 3. Gear System (4 Sets)

| Set | Weapon | Armor | Requires |
|-----|--------|-------|----------|
| **Starter** | Wooden Sword | Cloth Tunic | Starter Crop ×10 |
| **Boss A** | Boss A Weapon | Boss A Armor | Boss A Crop ×5 + Boss A Material ×3 |
| **Boss B** | Boss B Weapon | Boss B Armor | Boss B Crop ×5 + Boss B Material ×3 |
| **Master** | Master Weapon | Master Armor | All Crops ×5 + All Materials ×2 |

- Each set: 1 weapon (affects attack pattern/damage) + 1 armor (affects HP/defense)
- Crafting UI shows available recipes based on inventory
- No currency — pure material gating

## 4. Boss Design

| Boss | Access | Patterns | Drops |
|------|--------|----------|-------|
| **Boss A** | Day 1 | 3 patterns, 2 phases | Boss A Material, unlocks Boss A Crop |
| **Boss B** | Day 1 | 3 patterns, 2 phases | Boss B Material, unlocks Boss B Crop |
| **Dual Boss** | After A+B beaten | Combined patterns, shared HP | Victory |

## 5. Development Workflow

### Commands
```bash
# Native dev (primary iteration)
cargo run

# WASM test build (every 2-3 hours)
cargo build --target wasm32-unknown-unknown --profile wasm-release
wasm-bindgen --out-dir dist --target web target/wasm32-unknown-unknown/wasm-release/dl_jam.wasm
# Test locally: python3 -m http.server 8000 -d dist

# Code quality
cargo check && cargo clippy && cargo fmt --check
```

## 6. 9-Day Milestone Plan

| Day | Date | Focus | Exit Criteria |
|-----|------|-------|---------------|
| 1 | Oct 3 | **Core: Movement + Pot Grid + Starter Crop** | 9 pots, plant/water/harvest starter crop, 3 growth stages |
| 2 | Oct 4 | **Gear: Starter Set + Crafting UI** | Craft Wooden Sword + Cloth Tunic from starter crops |
| 3 | Oct 5 | **Boss A: Arena, Patterns, Drops** | Beatable Boss A, drops material, unlocks Boss A crop |
| 4 | Oct 6 | **Boss B + Day Cycle** | Boss B beatable, Day/Night loop works, both crops unlock |
| 5 | Oct 7 | **Gear Sets 2 & 3 + Dual Boss Unlock** | All 4 gear sets craftable, Dual Boss unlocks after A+B |
| 6 | Oct 8 | **Dual Boss + Assets Integration** | Dual Boss fightable, all Dreamlayer assets loaded |
| 7 | Oct 9 | **Polish & Juice** | Particles, screen shake, sound, UI transitions, balance |
| 8 | Oct 10 | **WASM + itch.io + Save/Load** | Clean WASM builds, itch page, SaveManager implemented |
| 9 | Oct 11 | **Buffer + Submit** | Final build by 21:30 |

### Daily WASM Test (20:00)
Build and test web version to catch WASM-specific issues early.

## 7. Asset Pipeline

### Directory Structure
```
assets/
├── images/
│   ├── crops/        # 3 types × 3 stages = 9 sprites
│   ├── gear/         # 4 sets × (weapon + armor) = 8 sprites
│   ├── bosses/       # Boss A, Boss B, Dual (spritesheets)
│   ├── ui/           # panels, buttons, icons, pot frames
│   └── tiles/        # floor, walls, arena
├── audio/
│   ├── sfx/          # .ogg (plant, water, harvest, craft, hit, boss)
│   └── music/        # .ogg (farm_day, boss_night, victory)
└── fonts/
    └── .ttf
```

### Loading
- `AssetServer::load_folder("images")` + `load_folder("audio")` in `LoadingAssets` state
- `TextureAtlasLayout` for animated sprites (crops, bosses)
- Audio: OGG Vorbis, mono SFX, stereo music, <2MB total

## 8. Input Scheme (Desktop WASM)

| Action | Key/Mouse |
|--------|-----------|
| Move | WASD / Arrow Keys |
| Interact (plant/water/harvest/craft/attack) | Left Click / Space |
| Gear Swap | 1 (weapon), 2 (armor) / Scroll |
| Open Crafting | Tab / C |
| Boss Select | Click UI |
| Pause | Escape |

## 9. Persistence Architecture (Save/Load Ready)

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
- Stub `SaveManager` Day 4, implement backends Day 8-9

## 10. Performance (WASM)
- Texture atlases (max 2048×2048, pack crops/gear/UI separately)
- `FixedTimestep(60Hz)` for game logic
- `Query` filters: prefer `With<>`/`Without<>` over `Changed<>`
- Profile: `cargo build --profile wasm-release` + `wasm-opt -Oz`
- Target: <5MB WASM, <10MB total assets

## 11. Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Boss patterns too complex | Start with 1 pattern per boss, add incrementally |
| Dreamlayer assets delayed | Kenney.nl placeholders in `assets/placeholder/` |
| WASM bugs late | Daily WASM test from Day 1 |
| Scope creep | Hard cap: 3 crops, 4 gear sets, 2+1 bosses |
| Save/Load time | Stub Day 4, implement Day 8-9 only if time |
| Dual boss balance | Test with max gear (Master set) as baseline |

## 12. File Structure (src/)
```
src/
├── main.rs                 # App entry, plugin registration
├── states.rs               # GameState, DayPhase
├── plugins/
│   ├── farm.rs
│   ├── gear.rs
│   ├── boss.rs
│   ├── day_cycle.rs
│   ├── persistence.rs
│   ├── ui.rs
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
│   └── save_manager.rs
├── events.rs
├── systems/
│   ├── farming.rs
│   ├── crafting.rs
│   ├── combat.rs
│   └── day_cycle.rs
└── utils/
    └── save_backend.rs
```

---

## 13. Git Policy
- **Never** run git commands requiring SSH or GPG (push, commit, tag, etc.)
- `git commit` is configured to always require signing — do not attempt commits
- `git add` should also not be used
- Only safe commands: `git status`, `git diff`, `git log`

*Generated for Dreamlayer Game Jam — Oct 3-11, 2026*