# Visual Style Guide — dl-jam

## Core Aesthetic
**"Lived-in modern fantasy"** — utilitarian, worn-in, familiar danger. Not cozy, not grimdark. Comfort earned through survival.

---

## Color Palette (Desaturated 15–20%)

| Role | Hex (Target) | Description |
|------|--------------|-------------|
| **Sickly teal / Mercury sheen** | `#9cf6f6` → ~`#7dc8c8` | Mercury-tainted water glow, vial contents, micro-highlights on gear/eyes |
| **Dusty ochre / Tailings dust** | `#f3c98b` → ~`#c4a36f` | Tailings dust, dried mud, warm highlights, underlayers |
| **Muted clay / Sickly clay** | `#daa588` → ~`#a8836e` | Skin undertones, worn leather, wooden shafts, boots/bracers |
| **Deep mine shadow** | `#011936` → ~`#001428` | Night, deep water, heavy shadows, coat base |
| **Scarf red (only saturated warm color)** | `#7a1f1f` | **Only** pure warm hue — dried blood/oxidized iron, life, warning |

**Rule**: Red appears *only* on the scarf. Tiny mercury-silver accents allowed if functionally justified (e.g., edge highlights, eye catchlights).

### Winter Variant Accent Palettes (Desaturated, within base palette)

| Variant | Primary Accent | Secondary Accent | Usage |
|---------|----------------|------------------|-------|
| **Ember Mail** | Burnt orange `#a64b1a` | Ash grey `#5a5a5a` | Metal plates, leather, fabric patches |
| **Gloom Robe** | Mercury silver `#9cf6f6` | Deep indigo `#0a0a2e` | Oilcloth panels, filigree, beads, collar |
| **Layered Aegis** | All combined | All combined | Modular scales, integrated webbing, all ward metals |

---

## Character Design

### Protagonist
- **Androgynous**, medium-length hair messily tied back, loose strands framing face
- **Scarf**: Long, plain, slightly tattered, **#7a1f1f** (dried blood/oxidized iron) — only saturated warm color
- **Spearblade**: Two-handed, handmade, ~character height. Wooden shaft, dull pitted metal blade, leather bindings stained dark from contaminated materials. Blade runs along shaft for maneuverability.
- **Outfit (day/greenhouse)**: Layered stain-resistant fabrics, rolled sleeves, simple trousers/shorts, soft boots, apron, gloves, seed pouches (some holding sealed vials — faintly glowing sickly teal, others inert grey sludge)
- **Wear details**: Patched fabric, scuffed boots, repaired gloves, visible repairs, cuffs/collar/boot tops dusted with fine grey tailings residue
- **Expression**: Hollow calm, pupils slightly dilated (mercury neurological tell)
- **Corruption**: Environmental only — stained bindings, tailings dust on cuffs, pitted blade, sealed vials, dilated pupils. **No veins, no mutations, no body horror.**

### Gear Sets (4 total, per-piece crafting)
| Set | Weapon | Armor | Visual Identity |
|-----|--------|-------|-----------------|
| **Starter** | Starter Spearblade | Cloth Tunic | Plain wood, simple bindings, undyed cloth |
| **Boss A** | Spark Spearblade | Ember Mail | Metal gleam, ember accents, heat-discoloration |
| **Boss B** | Shadow Spearblade | Gloom Robe | Dark patina, matte finish, shadowy aura |
| **Master** | Dreaming Spearblade | Layered Aegis | Refined, layered materials, subtle iridescence |

### Winter Combat Variants (2-view: front + left profile)
All variants share: same character (androgynous, grey eyes, medium hair tied back, #7a1f1f scarf waist-length), plain light grey backdrop, muted palette throughout, no weapons (hands empty), folk wards sewn into workwear, all fabrics fully opaque.

| Variant | Gear Set | Theme | Silhouette | Palette Accents | Folk Wards |
|---------|----------|-------|------------|-----------------|------------|
| **Winter Base** | — | Gardener winter combat | Insulated coat #011936, reinforced patches, cold-iron stitching, ash wood toggles | Base only | Cold iron, ash wood, red thread, rowan |
| **Ember Mail** | Boss A | Fire/ash/forge | Metal-reinforced shoulder/forearm plates, furnace-breathed leather, ash-warded fabric | Burnt orange #a64b1a, ash grey #5a5a5a (desaturated) | + Copper wire, copper buckles |
| **Gloom Robe** | Boss B | Shadow/mercury/void | **Asymmetric wrap coat** — right over left, silver toggles at hip, left panel longer with split vent, standing mercury-stiffened collar | **Mercury silver #9cf6f6 as deliberate design lines** (full front panel, sleeve insets, collar lining, hem binding), deep indigo #0a0a2e | + Silver filigree, silver bezel hagstone, silver mesh salt pouches, liquid mercury beads, blackthorn |
| **Layered Aegis** | Master | Synthesis/masterwork | Heaviest: modular plate scales over coat, integrated harness-webbing, articulated pauldron/fauld | All combined (desaturated): mercury, dried blood, ash, clay | **All wards densest integration**: cold iron, copper, silver, hagstone, salt, blackthorn, rowan, ash wood, red thread |

### Design Language: "Folklore Meets Industrial Toxicity in Winter"
Protective charms are functional workwear details — not jewelry. The hazard lives in the gear, the stains, the wards.

---

## World Atmosphere

**Darker undertones** beneath the lived-in surface:
- Caustic tailings on snow (grey-green mineral crust)
- Strange creatures woken from a mining disaster
- Gentle madness of mercury in the water
- Fine tailings dust everywhere — on clothes, in air, on crops
- Sealed vials in pouches: some glow faintly sickly, others are inert grey sludge

---

## Sprite Specifications

### Base Frame
- **Size**: 64×96 px (w×h) — accommodates 32×32 collision + spear on back + headroom
- **Pivot**: Bottom-center (feet at y=0)
- **Atlas**: 4 rows (down/up/left/right) × N columns

### Directions (3-angle source)
| Logical | Source |
|---------|--------|
| Down (South) | Front |
| Up (North) | Front (vertically flipped) — *dedicated back if added later* |
| Left (West) | Left |
| Right (East) | Right (or mirrored Left) |

### Animations (Gardening Outfit)
| Animation | Frames | Loop | Notes |
|-----------|--------|------|-------|
| **Idle** | 4 | ✓ | Breathing, weight shifts |
| **Walk** | 8 | ✓ | Full cycle, spear bobs on back |
| **Interact** | 6 | ✗ | Generic "action" — water/harvest/plant |
| **Carry** | 1 | ✓ | Overlay — holding crop/watering can |

**Total**: ~25–30 unique frames × 3 angles = 75–90 frames

### Spear Handling
- **Farming**: Sheathed on back (child entity, toggled visible)
- **Boss fight**: Drawn in hand (swaps to drawn overlay)
- **Component**: `SpearState { sheathed: bool }`

### Weapon Overlays (4 sets × 2 states)
- `starter_sheathed/drawn`, `boss_a_sheathed/drawn`, `boss_b_sheathed/drawn`, `master_sheathed/drawn`

---
---

## UI / UX Visuals
- **Panels**: Stained, patched textures — same materials as clothing
- **Text**: High contrast against dark backgrounds, no pure white (use dusty ochre)
- **Icons**: Simple, worn — scratched metal, carved wood
- **Health bars**: Mine shadow background, sickly teal fill, scarf red warning at low HP

---

## Lighting / Rendering
- **Farm (day)**: Flat, overcast — muted, even lighting
- **Arena (night)**: Single directional source (moon/bioluminescence), deep shadows
- **No bloom, no neon, no HDR** — deliberate, painted feel
- **Palette clamping**: Shader enforces desaturation on all sprites except scarf red

---

## Audio Palette (Reference)
- **SFX**: Mono OGG, gritty, tactile — wet soil, dry wood, metal-on-stone
- **Music**: Stereo OGG, minimal — farm: sparse guitar/ambient; arena: low drone/percussion; victory: resolved version of farm theme
- **No chiptune, no orchestral** — lo-fi, intimate, analogue

---

## Anti-Patterns (Do Not Use)
- ❌ Heavy plate armor
- ❌ Oversized anime weapons
- ❌ Ornate royal clothing
- ❌ Full plant costume / leaf armor / flower crown
- ❌ Pristine brand-new gear
- ❌ Face-covering helmets
- ❌ Grimdark gore / body horror
- ❌ Sci-fi neon overload
- ❌ Modern guns
- ❌ Overly gendered design
- ❌ Pure white highlights
- ❌ Saturated colors (except scarf red #7a1f1f)
- ❌ Transparent / ghosted / semi-transparent fabrics
- ❌ Scarf tucked under coat / disappearing into dark fabric
- ❌ Scarf design drift from base #7a1f1f tattered wool reference

---

## Generation Prompts (Reference)

### Character Turnaround (3-angle)
```
Character turnaround reference sheet: 3 views arranged horizontally — fully front, left profile, right profile. Same character design: androgynous, medium hair tied back, tattered red scarf #7a1f1f, handmade spearblade (wooden shaft, pitted metal blade, leather bindings stained dark), lived-in greenhouse outfit (apron, gloves, seed pouches with sealed vials). Plain neutral backdrop. Consistent proportions, lighting, and style across all 3 views. Palette desaturated 15–20%: sickly pale teal, dusty ochre, muted clay, deep mine shadow; red only on scarf #7a1f1f. No heavy plate armor, no oversized anime weapon, no ornate royal clothing, no full plant costume/leaf armor/flower crown, no pristine gear, no face-covering helmet, no grimdark gore, no sci-fi neon, no modern guns, no overly gendered design.
```

### Sprite Sheet (per animation)
```
Sprite sheet, 8 frames horizontal, 64x96 each: [animation name] cycle for androgynous farmer in lived-in greenhouse outfit (apron, gloves, rolled sleeves, seed pouches), spear sheathed on back. [Direction] view. Plain backdrop. Palette: desaturated sickly teal, dusty ochre, muted clay, deep mine shadow; only saturated color is scarf red #7a1f1f. Worn details: patched fabric, scuffed boots, tailings dust on cuffs. No grimdark, no neon, no pristine gear.
```

### Winter Combat Variants (2-view: front + left, image_to_image from winter_base reference)
**Reference**: `assets/images/character/winter/base/turnaround_2view.png` (winter base)

#### Ember Mail (Boss A)
```
Character turnaround sheet, 2 views: front, left profile. Plain light grey backdrop. Androgynous gardener in Ember Mail winter combat gear. Medium-length hair messily tied back, loose strands framing face, grey eyes. Scarf: #7a1f1f dried blood tattered wool, waist-length, substantial volume, sits clearly on top of coat collar with sharp opaque edge — matches base winter outfit exactly. Coat: #011936 deep mine water with metal-reinforced shoulder/forearm plates, furnace-breathed leather, ash-warded fabric patches. Underlayers: #f3c98b tailings dust. Bracers/boots: #daa588 sickly clay leather. Gloves: cold-iron and copper stitching at seams. Harness: ash wood toggles, cold iron + copper buckles, red thread stitching at stress points. Belt pouches: rowan twigs, copper wire in flaps. Wear: patched fabric, scuffed boots, repaired gloves, faint chemical staining. All fabrics fully opaque, solid. Palette: base + burnt orange #a64b1a, ash grey #5a5a5a desaturated only. Mercury sheen #9cf6f6 micro-highlights only. No weapons, hands empty. Folk wards: cold iron, copper, ash wood, rowan, red thread. Folklore meets industrial toxicity in winter. Muted throughout.
```

#### Gloom Robe (Boss B)
```
Character turnaround sheet, 2 views: front, left profile. Plain light grey backdrop. Androgynous gardener in Gloom Robe winter combat gear. Medium-length hair messily tied back, loose strands framing face, grey eyes. Scarf: #7a1f1f dried blood tattered wool, waist-length, substantial volume, sits clearly on top of coat collar with sharp opaque edge — matches base winter outfit exactly. Coat: #011936 deep mine water, asymmetric mercury-slick oilcloth wrap coat — right panel crosses over left, secured with visible silver toggles at hip, left panel longer with split vent at hem, structured draping, integrated standing collar (solid opaque fabric, mercury-stiffened). Mercury-slick oilcloth as deliberate design lines: full front wrap panel, sleeve insets, collar lining, hem binding — all catching light as distinct silver surfaces (#9cf6f6). Underlayers: #f3c98b tailings dust visible at vent/split. Bracers/boots: #daa588 sickly clay leather with silver thread. Gloves: cold-iron and silver stitching at seams. Harness: ash wood toggles, cold iron + silver buckles, silver filigree wire in harness webbing at joints. Hagstone at throat in silver bezel (prominent, distinct). Red thread stitching (distinct from scarf) at stress points. Belt pouches: rowan twigs, salt packets in silver mesh pouches, blackthorn visible in flaps. Liquid mercury beads at cuff ends, hem points, pouch flap corners. Wear: patched fabric, scuffed boots, repaired gloves, faint chemical staining on cuffs/hems. All fabrics fully opaque, solid, no transparency/ghosting anywhere. Palette: base + mercury silver #9cf6f6 as deliberate design lines throughout, deep indigo #0a0a2e desaturated only in shadows. No weapons, hands empty at sides. Folk wards: cold iron, silver, hagstone, salt, blackthorn, rowan, ash wood, red thread. Folklore meets industrial toxicity in winter. Muted throughout but silver reads clearly.
```

#### Layered Aegis (Master)
```
Character turnaround sheet, 2 views: front, left profile. Plain light grey backdrop. Androgynous gardener in Layered Aegis winter combat gear. Medium-length hair messily tied back, loose strands framing face, grey eyes. Scarf: #7a1f1f dried blood tattered wool, waist-length, substantial volume, sits clearly on top of coat collar with sharp opaque edge — matches base winter outfit exactly. Heaviest modular silhouette: coat in #011936 with articulated plate scales over shoulders/pauldrons/faulds, integrated harness-webbing, warm underlayers in #f3c98b, multi-metal leather bracers and boots in #daa588, snow boots, gloves with cold-iron, copper, and silver stitching at the seams. Harness with ash wood toggles, cold iron, copper, and silver buckles, hagstone at throat, red thread stitching (distinct from scarf) at every stress point. Seed pouches at belt with rowan twigs, copper wire, silver thread, salt packets, blackthorn in pouch flaps. Wear: patched coat, scuffed boots, repaired gloves, visible repairs, faint chemical staining on cuffs and hems. Colors remain muted throughout — all accent tones (burnt orange, mercury silver, deep indigo, dried blood red) desaturated into the base palette, mercury sheen (#9cf6f6) only as barely-there highlights on highest edges. All fabrics fully opaque, solid. No weapons, hands empty at sides. Folk wards: densest integration — rowan, cold iron, copper, silver, hagstone, salt, blackthorn, ash wood, red thread all present. Folklore meets industrial toxicity in winter. Muted throughout.
```

---

*Last updated: 2026-10-08*

---

## Boss Visual Design (Added 2026-10-08)

### Boss A — The Excavator (Excavator-spritesheet)
**Concept**: Industrial mining excavator fused with the miners it crushed. More machine than flesh — heavy caterpillar treads replace legs, hydraulic piston arms end in articulated pickaxe-claw manipulators. Operator's cabin is a reinforced steel cage with cracked reinforced glass; a miner's helmet mounted externally as a memorial. Hydraulic hoses run along the frame like exposed architecture, some weeping sickly yellow-green caustic fluid (#9cf6f6) from corroded fittings. Rusted steel plates patched with organic-looking copper-green corrosion spreading like lichen across seams. Faint yellow-green (#9cf6f6) bioluminescent residue pools in recessed panel seams and hydraulic joints — subtle glow, not veiny. Miner's helmet mounted on the cabin roof as a memorial marker. **Palette**: rusted steel, corroded copper-green patina, sickly yellow-green residue glow (#9cf6f6) in recessed areas only, deep navy (#011936) shadows. Red (#b23a48) ONLY as tiny hazard strip accents.

**Animations (8-direction isometric packs, 5×5 grids = 25 frames each)**:
| Animation | Behavior | Direction Constraint |
|-----------|----------|---------------------|
| Idle | Breathing, hydraulic hiss | Full 8-dir |
| Walk | Tread grind, piston shift | Full 8-dir |
| **Tailings Surge** | Windup → charge, leaves caustic trail | **Horizontal ONLY (snaps to pure Left/Right)** |
| **Excavator Slam** | Windup → AoE slam, acid pools in P2 | **Never Left/Right (snaps to Up/Down)** |
| Debris Rain | Telegraph → falling ore chunks | Full 8-dir |
| Death (Tip over) | Slow topple, holds last frame | Full 8-dir |

**Frame counts**: 5×5 grid (25 frames) for all except the death clip uses all 25 frames.

---

### Boss B — The Quicksilver / Mercuril (Mercuril-spritesheet)
**Concept**: Protean entity of living mercury with NO consistent silhouette — constantly shifting between geometric forms: spheres flattening into sheets, tendrils branching into fractal tendrils, cubes melting into spheres, sheets folding into impossible polyhedra. **NO humanoid silhouette ever.** Surface is perfect mirror-chrome mercury reflecting a distorted version of the surroundings. No face, no features — just a seamless reflective surface. Thin threads of deep crimson (#b23a48) — "madness" — trace through the mercury like circuit traces, pulsing in impossible patterns. Mercury constantly fractures into perfect droplets that orbit and rejoin, some becoming independent geometric polyhedra that orbit and rejoin. Wet metallic surface catches light with perfect specular highlights. **Palette**: mirror chrome, perfect silver reflections, deep crimson (#b23a48) geometric thread traces pulsing in impossible patterns, void black (#011936) negative space. Red (#b23a48) ONLY as the madness circuitry.

**Animations (8-direction isometric packs, 5×5 grids = 25 frames each, except death)**:
| Animation | Behavior | Direction Constraint |
|-----------|----------|---------------------|
| Idle | Constant subtle shifting | Full 8-dir |
| Walk | Fluid glide, no limbs | Full 8-dir |
| Mirror Step | Blink + decoys | Full 8-dir |
| Quicksilver Wave | Travelling mercury wall | Full 8-dir |
| Madness Spray | Cone/sphere burst, homing wisps in P2 | Full 8-dir |
| **Death (turn into goo)** | Dissolves into pool | **Mercuril only: first 12 frames of 5×5 grid (5×3 = 15 drawn, use 12)** |

**Frame counts**: 5×5 grid (25 frames) for all clips. **Exception**: Mercuril death ("turn into goo") — art only fills first 12 frames (5×3 rows). Animation system caps at 12 frames for this pack/state.

---

### Dual Boss — The Disaster (Arena Dual)
Both halves fight simultaneously in the same arena, sharing a single 900 HP pool.
- **Excavator half** (left spawn): Uses Excavator pack, Excavator patterns
- **Quicksilver half** (right spawn): Uses Mercuril pack, Quicksilver patterns
- **Coordinated combos** (every ~6.4s): 
  - Surge + Step, Slam + Wave, Rain + Spray
- **Amalgamation** (every 20–30s): Both converge center, 5s channel → room-filling purple (#b23a48) explosion; player must hide behind central pillar (LOS check)
- **Death**: Both halves play their respective death clips simultaneously; defeat fires when both finish.

---

### Animation Constraints (Implemented in Code)

| Constraint                           | Implementation                                                                                                                            |
|--------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------|
| **Mercuril death = 12 frames**       | `BossAnimState::Death.max_frames(Mercuril) == 12` — one-shot clips clamp at drawn frame count                                             |
| **Tailings Surge horizontal-only**   | `constrained_aim` snaps charge direction to pure Left/Right; applied to charger aim, second-charge re-aim, and animation facing           |
| **Excavator Slam avoids Left/Right** | `constrained_aim` snaps slam facing to vertical when aim lands in left/right band (±22.5°); diagonals and vertical pass through unchanged |

**Death animation**: On lethal hit, `Dying` component inserted with `death_duration(pack)` timer. `drive_boss_animation` forces Death state. `tick_dying_bosses` runs timer, fires `BossDefeated`/despawn on expiry. Dual halves die together, each playing its own sheet; coordinator stops on dual death.

---

### Plants

| Plant            | Discription                                                                                                                                     |
|------------------|-------------------------------------------------------------------------------------------------------------------------------------------------|
| Quicksilver Reed | A fastgrowing reed, rich in murcury. It can be woven into a sturdy cloth if dried properly.                                                     |
| Cinder Cap       | A fly Agaric mushroom, doing its best to remove toxins from the soil. As a side effect, its spores are incredibly flamable.                     |
| Tailings Potato  | A standard potato. Its tubers are filled to bursting with heavy metals, and can be refined into a sturdy armor in the hands of a skilled smith. |

---

## Plant Concept Art Generation Status (2026-10-08)

### Generated Files (in `assets/reference_images/plants/`)

| File | Stage | Method | Status |
|------|-------|--------|--------|
| `quicksilver_reed_seedling.png` | Quicksilver Reed — Seedling | Text-to-Image | ✓ Done |
| `cinder_cap_seedling.png` | Cinder Cap — Seedling (pure white puffball) | Text-to-Image | ✓ Done |
| `tailings_potato_seedling.png` | Tailings Potato — Seedling | Text-to-Image | ✓ Done |

**Pending (image-to-image failing with 503 service errors):**
- Quicksilver Reed — Grown (from seedling_v2)
- Cinder Cap — Growing (from seedling_v2)
- Cinder Cap — Grown (from growing)
- Tailings Potato — Growing Small (from seedling)
- Tailings Potato — Growing Large (from growing small)
- Tailings Potato — Grown (from growing large)

---

## Plant Generation Prompts (Reference)

### Shared Style Constraints (all prompts)
- **Transparent PNG background** (alpha channel)
- Isometric perspective (2:1 dimetric)
- Simple mound of dirt: primarily muted clay brown (`#a8836e`) and dusty ochre (`#c4a36f`) with deep mine shadow (`#001428`) undertones
- Small contamination patches only: sparse mercury teal (`#7dc8c8`), tailings orange-yellow, faint red/blue/green mineral specks — subtle, not dominant
- Desaturated palette (15–20%)
- Only saturated color: scarf red `#7a1f1f` as microscopic folk-ward accents only
- Fully opaque, no transparency/ghosting
- Utilitarian, worn, lived-in feel

---

### Quicksilver Reed (Starter Crop) — 2 Stages

**Growth time**: 1 day → Seedling + Grown

#### Stage 1: Seedling — Text-to-Image ✓
```
Isometric concept art on plain white background: Quicksilver Reed seedling emerging from a simple mound of primarily dirt-brown soil. Soil mound base: muted clay brown (#a8836e) and dusty ochre (#c4a36f) with deep mine shadow (#001428) undertones. Small contamination patches only: sparse flecks of mercury teal (#7dc8c8), tailings orange-yellow, faint red/blue/green mineral specks -- subtle, not dominant. Two to three fragile straight upright pale-teal shoots (#7dc8c8) barely 3 inches tall, straight and vertical like young grass blades, not curled or coiled. Stems show faint mercury-silver micro-highlights (#9cf6f6) along edges. A microscopic red thread folk ward (#7a1f1f) tied at base of one shoot. Utilitarian, worn, no fantasy glow. Clean isometric view, no cast shadows on white.
```

#### Stage 2: Grown (Harvestable) — Image-to-Image (from seedling_v2) ⏳
```
Isometric concept art on plain white background: Mature Quicksilver Reed ready for harvest, rising from the same simple soil mound. Soil mound base: muted clay brown (#a8836e) and dusty ochre (#c4a36f) with deep mine shadow (#001428) undertones. Small contamination patches only: sparse mercury teal (#7dc8c8), tailings orange-yellow, faint red/blue/green mineral flecks. 5–7 slender vertical stems (~18 inches tall) in desaturated mercury teal (#7dc8c8) with distinct mercury-silver edge highlights (#9cf6f6) catching light. Stems hollow, segmented like fine bamboo, faint internal glow at joints. Reduced papery leaf sheaths at nodes in tailings ochre (#c4a36f). Soil mound cracked, stained with mercury residue. A small woven fiber sample lies beside the mound -- pale translucent cloth from dried reeds showing end-use material. Folk wards (red thread #7a1f1f + copper wire) at mound base. Utilitarian, lived-in. Clean isometric view. [Use prior seedling image as reference for soil mound shape and perspective]
```

---

### Cinder Cap (Boss A Crop) — 3 Stages

**Growth time**: 2 days → Seedling + Growing + Grown

#### Stage 1: Seedling — Text-to-Image ✓ (pure white puffball button)
```
Isometric concept art on plain white background: Cinder Cap mushroom seedling (fly agaric button stage) pushing through a simple mound of primarily dirt-brown soil. Soil mound base: muted clay brown (#a8836e) and dusty ochre (#c4a36f) with deep mine shadow (#001428) undertones. Small contamination patches only: sparse flecks of mercury teal (#7dc8c8), tailings orange-yellow, faint red/blue/green mineral specks -- subtle, not dominant. Cap is a pure white puffball-like sphere ~1 inch diameter, smooth and featureless, resembling a young puffball or button mushroom before veil breaks. No colored warts or scales yet. Thick stem in tailings ochre (#c4a36f), swollen at base, partially buried. Gills not yet visible (universal veil intact). Soil mound crusted with grey-green tailings mineral residue. A faint heat shimmer (barely visible desaturated burnt orange #a64b1a) rises from cap -- hinting at flammable spores. Single copper wire folk ward twisted into mound. No saturated colors. Clean isometric view.
```

#### Stage 2: Growing (Day 1–2) — Image-to-Image (from seedling_v2) ⏳
```
Isometric concept art on plain white background: Adolescent Cinder Cap expanding from the same soil mound. Cap ~3 inches diameter, classic fly agaric shape -- desaturated brick (#a8836e) with scattered ochre wart-scales (#c4a36f). Gills beginning to show at margin, pale with faint sickly teal luminescence (#7dc8c8) deep in folds (mercury-tainted spores). Stem 4 inches tall, thick, fragile skirt (annulus) dusted in ochre spores. Basal bulb pronounced, mycelial threads (mercury silver #9cf6f6) visible spreading across mound surface. Heat shimmer more visible (desaturated burnt orange #a64b1a distortion). Copper and cold-iron folk wards at mound edge. Soil mound heat-cracked. Clean isometric view. [Use prior seedling image as reference for mound shape and perspective]
```

#### Stage 3: Grown (Harvestable) — Image-to-Image (from growing) ⏳
```
Isometric concept art on plain white background: Fully mature Cinder Cap, cap fully expanded ~5 inches with upturned margin revealing crowded gills. Cap: desaturated fly agaric brick (#a8836e) with prominent ochre wart-scales (#c4a36f). Gills deep, pale, actively releasing fine spore haze -- sickly teal (#7dc8c8) and ochre (#c4a36f) particles catching light. Stem sturdy, 6 inches, basal bulb pronounced, ring remnant hanging. Mycelium dense at mound surface as mercury-silver threads (#9cf6f6). Soil mound heavily heat-cracked, folk wards dense (copper wire, red thread #7a1f1f, rowan twig). Leather gloves and sealed tin beside mound -- harvesting gear for flammable spores. Utilitarian, hazardous but routine. Clean isometric view. [Use prior growing image as reference for mound shape and perspective]
```

---

### Tailings Potato (Boss B Crop) — 4 Stages

**Growth time**: 3 days → Seedling + Growing Small + Growing Large + Grown

#### Stage 1: Seedling — Text-to-Image ✓
```
Isometric concept art on plain white background: Tailings Potato seedling -- single sturdy shoot emerging from a simple mound of primarily dirt-brown soil. Soil mound base: muted clay brown (#a8836e) and dusty ochre (#c4a36f) with deep mine shadow (#001428) undertones. Small contamination patches only: sparse flecks of mercury teal (#7dc8c8), tailings orange-yellow, faint red/blue/green mineral specks -- subtle, not dominant. Stem thick, muted clay green (#a8836e) with mercury-silver stippling (#9cf6f6) at nodes. Two compound leaves unfurling, leaflets broad, undersides tailings ochre (#c4a36f). A small sealed assay vial (inert grey sludge) stuck in mound beside shoot -- marker. Cold-iron nail driven into mound edge as folk ward. No saturated colors. Clean isometric view.
```

#### Stage 2: Growing Small (Day 1) — Image-to-Image (from seedling) ⏳
```
Isometric concept art on plain white background: Young Tailings Potato plant ~8 inches tall, 4–5 main stems from central crown on the same soil mound. Stems thick, succulent, muted clay (#a8836e) with mercury-silver stippling (#9cf6f6). Compound leaves broad, ochre undersides (#c4a36f), veins faintly mercury-teal (#7dc8c8). Mound surface cracking, revealing tuber swellings at stem bases -- dark, dense, heavy-metal-laden. Folk wards added: red thread (#7a1f1f) and hagstone chip at mound rim. Soil mound crust thickening. Clean isometric view. [Use prior seedling image as reference for mound shape and perspective]
```

#### Stage 3: Growing Large (Day 2–3) — Image-to-Image (from growing small) ⏳
```
Isometric concept art on plain white background: Robust Tailings Potato plant ~14 inches tall, dense foliage canopy from same mound. Stems thickened, mercury-silver stippling (#9cf6f6) pronounced. Leaves large, leathery, muted clay (#a8836e) with ochre veins (#c4a36f). Mound heavily distended -- large tuber shoulders breaking soil surface, dark and dense, mercury-silver veining (#9cf6f6) visible on exposed tuber skin. Folk wards dense: cold-iron nails, copper wire, red thread (#7a1f1f), blackthorn twigs at mound rim. A small refining hammer and tongs rest beside mound -- hinting at armor-smithing end use. Clean isometric view. [Use prior growing-small image as reference for mound shape and perspective]
```

#### Stage 4: Grown (Harvestable) — Image-to-Image (from growing large) ⏳
```
Isometric concept art on plain white background: Fully mature Tailings Potato, foliage beginning to yellow (harvest signal), same mound. Stems woody at base, mercury-silver stippling (#9cf6f6) heavy. Mound split open by massive tuber cluster -- 3–4 huge tubers fused at crown, each fist-sized, dark skin with brilliant mercury-silver veining (#9cf6f6) like circuitry. Tubers dense, heavy-metal-laden, faintly warm to touch (implied). Folk wards maximum density: cold iron, copper, silver, hagstone, blackthorn, red thread (#7a1f1f), rowan at mound rim. Refining tools beside mound: hammer, tongs, crucible. Utilitarian, industrial, routine hazard. Clean isometric view. [Use prior growing-large image as reference for mound shape and perspective]
```
