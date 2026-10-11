# Last Day Plan — dl-jam

**Core loop complete, 867 unit + 32 integration tests passing.**  
Focus: high-impact, low-risk work across four areas.

---

## Priority Order

1. **Save/Load** — critical for roguelite persistence
2. **Polish/Juice** — biggest "feel" wins per hour
3. **Accessibility/QOL** — low effort, broadens audience
4. **Infinite Mode** — post-game content, scoped to MVP

---

## 1. Save/Load

**Architecture exists:** `SaveManager` trait, all resources `Serialize/Deserialize/Reflect`, auto-save events already emitted (`DayAdvanced`, `BossDefeated`, `PlayerDied`, `GearCrafted`).

### Tasks
- `FileBackend` (native) — `bincode` → `save.dat` in platform data dir
- `IndexedDBBackend` (WASM) — `wasm-bindgen` + `web-sys` `IndexedDb`, async via `wasm-bindgen-futures`
- `SaveManager` resource with target detection (`cfg(target_arch = "wasm32")`)
- Wire auto-save: events → `SaveManager.save()`
- Load check in `LoadingAssets` → show "Continue" on `MainMenu` if save exists
- "Continue" flow: load → restore resources → `GameState::Playing`
- Test native + WASM round-trip

### Files
- `src/resources/save_manager.rs` (new)
- `src/utils/save_backend.rs` (extend)
- `src/plugins/persistence.rs` (new, wire into `GamePlugin`)
- `src/plugins/main_menu.rs` (add Continue button + load logic)

---

## 2. Polish/Juice

### Tasks
| Feature | Implementation |
|---------|----------------|
| **Screen shake** | `CameraShake` resource (trauma float); `Update` system decays + applies offset to camera. Trigger on `DamageDealt` (player hit), `BossAttackStarted` (big attacks), `BossDefeated`. |
| **Hit pause (frame freeze)** | `HitPause` resource (timer); in `FixedUpdate`, if >0, skip `grid_movement`/`boss_movement`/`boss_ai`. Trigger 0.05–0.1s on `HitConfirm`. |
| **Particle bursts** | Reuse `EffectSpriteAssets` pattern. `Particle { velocity, lifetime, gravity }` component + spawn system on `HitConfirm`/`CropHarvested`/`GearCrafted`. |
| **Crop growth pop** | Scale tween (0.8→1.1→1.0) on stage change; `CropSprite` already tracks stage. |
| **Boss telegraph flash** | Boss sprite outline flashes white during windup via `SpriteOutlineMaterial` `params.tint` animation. |
| **Audio mix pass** | Normalize volumes (music -12dB, SFX -6dB); add `master`/`music`/`sfx` volume resources; hook to pause menu sliders later. |
| **Damage numbers** | Already implemented — verify visibility, add crit color (yellow) for heavy attacks. |

### Files
- `src/resources/camera_shake.rs`, `src/resources/hit_pause.rs`
- `src/systems/particles.rs`
- `src/systems/camera_follow.rs` (apply shake)
- `src/plugins/audio.rs` (volumes)

---

## 3. Accessibility/QOL

### Tasks
| Feature | Implementation |
|---------|----------------|
| **Key rebinding** | `InputMap` resource (`HashMap<Action, Vec<KeyCode>>`); `Action` enum; Settings tab in pause overlay with click-to-rebind UI. Persist in save. |
| **Volume sliders** | `AudioSettings { master, music, sfx: f32 }`; apply in `AudioPlugin`; UI in Settings panel. |
| **Reduced motion** | `ReducedMotion` bool; disables camera shake, hit pause, particles, animated previews. Toggle in Settings. |
| **Colorblind-friendly palette** | `ColorblindMode: Off/Protanopia/Deuteranopia/Tritanopia`; swap `TEXT_CRAFTABLE`/`TEXT_BLOCKED`/`ROW_BORDER_SELECTED` via `ui_theme` palette lookup. Boss telegraphs already use pattern + shape. |
| **Text size scaling** | `TextScale` multiplier (0.75–1.5) in Settings, multiply `FontSize::Px`. |

### Files
- `src/resources/input_map.rs`, `src/resources/audio_settings.rs`, `src/resources/accessibility.rs`
- `src/plugins/pause.rs` (add Settings tab)
- `src/plugins/ui_theme.rs` (palette lookup)

---

## 4. Infinite Mode

**MVP:** Endless boss waves + local high score. No new assets.

### Tasks
- Unlock flag + menu entry: `BossProgress.infinite_unlocked` (set on dual victory); reveal "Infinite Mode" button in `BossSelectMenu` (reserved hidden slot exists)
- Wave generator: `InfiniteWave { index, boss_a_health_mult, boss_b_health_mult, damage_mult, pattern_pool }`; scales every 3 waves; alternate Boss A / B / Dual
- Wave transition: on `BossDefeated` in infinite, skip `Result` → `Loading` → next arena → `BossFight` (no farm day)
- Local high score: `InfiniteHighScore { wave, day, gear_set }` saved in run save; display on death/victory screen
- Infinite UI: wave counter in HUD (replaces day counter); "Endless" badge on boss select; death screen shows "Wave X reached"
- Balance test: play 10 waves, adjust scaling curve

### Files
- `src/resources/boss_progress.rs` (add flag)
- `src/resources/infinite_wave.rs` (new)
- `src/plugins/day_cycle.rs` (infinite branch)
- `src/plugins/hud.rs` (wave counter)
- `src/systems/boss_select.rs` (unlock logic)

---

## Scope Guards (Explicitly Not Doing)

- ❌ New sprite sheets / animations
- ❌ Online leaderboards
- ❌ New boss patterns
- ❌ Full settings menu rewrite
- ❌ Cloud save / Steam integration

---

## Quick Wins (Parallelizable, Start Anytime)

1. **Save/Load native backend** — pure Rust, no Bevy deps
2. **Screen shake + hit pause** — isolated systems, easy to test
3. **Volume resources + audio mix** — edit `AudioPlugin` constants
4. **Infinite mode flag + boss select unlock** — 1-line resource change + UI condition