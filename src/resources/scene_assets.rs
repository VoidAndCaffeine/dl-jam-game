use crate::components::boss::BossId;
use crate::components::boss_animation::{BossAnimState, BossPack};
use crate::components::effect_sprite::EffectKind;
use crate::components::gear::GearSet;
use crate::components::player_sprite::{Facing8, PlayerAnimState, PlayerLook};
use crate::levels::LevelId;
use crate::states::DayPhase;
use bevy::asset::LoadState;
use bevy::prelude::*;
use std::collections::HashMap;

/// The set of sheets a scene needs resident in memory.
///
/// A boss arena only needs the player's worn look plus the one boss it fights,
/// so the old scene's sheets can be dropped when the new one loads.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LoadTarget {
    /// The farm is always the farmer look and has no boss.
    Farm,
    /// The boss-select menu stays on the farm but preloads both bosses so the
    /// fight itself starts instantly.
    BossSelect,
    ArenaA,
    ArenaB,
    ArenaDual,
}

impl LoadTarget {
    /// The scene a given boss fight loads into.
    pub fn for_boss(id: BossId) -> Self {
        match id {
            BossId::BossA => LoadTarget::ArenaA,
            BossId::BossB => LoadTarget::ArenaB,
            BossId::Dual => LoadTarget::ArenaDual,
        }
    }

    /// The room this target shows. Boss select is a phase on the farm.
    pub fn level(self) -> LevelId {
        match self {
            LoadTarget::Farm | LoadTarget::BossSelect => LevelId::Farm,
            LoadTarget::ArenaA => LevelId::ArenaA,
            LoadTarget::ArenaB => LevelId::ArenaB,
            LoadTarget::ArenaDual => LevelId::ArenaDual,
        }
    }

    /// The player look worn in this scene.
    ///
    /// The farm always shows the farmer because only that pack has the planting
    /// and watering clips; arenas wear the equipped armor set.
    pub fn player_look(self, armor: Option<GearSet>) -> PlayerLook {
        match self {
            LoadTarget::Farm | LoadTarget::BossSelect => PlayerLook::Farmer,
            _ => armor.map_or(PlayerLook::Farmer, PlayerLook::from_gear),
        }
    }

    /// Human-readable name for the loading screen.
    pub fn label(self) -> &'static str {
        match self {
            LoadTarget::Farm => "the Farm",
            LoadTarget::BossSelect => "Boss Select",
            LoadTarget::ArenaA => "the Excavator Arena",
            LoadTarget::ArenaB => "the Quicksilver Arena",
            LoadTarget::ArenaDual => "the Dual Arena",
        }
    }

    /// Every sheet this scene needs, player look first then boss packs.
    pub fn required_paths(self, armor: Option<GearSet>) -> Vec<String> {
        let mut paths = player_paths(self.player_look(armor));
        match self {
            LoadTarget::Farm => {}
            LoadTarget::BossSelect => {
                // The menu is text-only, so a single idle frame per boss is
                // enough to back a future portrait without holding both full
                // packs (~1 GB decoded) just to open the menu. The chosen
                // boss's full pack loads on arena entry.
                for pack in [BossPack::Excavator, BossPack::Mercuril] {
                    paths.push(BossAnimState::Idle.sheet_path(pack, Facing8::Down));
                }
            }
            LoadTarget::ArenaA => {
                paths.extend(boss_paths(BossPack::Excavator));
                paths.extend(effect_paths(&[
                    EffectKind::TailingsSurge,
                    EffectKind::ExcavatorSlam,
                    EffectKind::AcidPool,
                    EffectKind::DebrisShadow,
                    EffectKind::DebrisImpact,
                ]));
            }
            LoadTarget::ArenaB => {
                paths.extend(boss_paths(BossPack::Mercuril));
                paths.extend(effect_paths(&[
                    EffectKind::QuicksilverWave,
                    EffectKind::MercuryPool,
                    EffectKind::MadnessSpray,
                    EffectKind::Wisp,
                ]));
            }
            LoadTarget::ArenaDual => {
                paths.extend(boss_paths(BossPack::Excavator));
                paths.extend(boss_paths(BossPack::Mercuril));
                paths.extend(effect_paths(&[
                    EffectKind::TailingsSurge,
                    EffectKind::ExcavatorSlam,
                    EffectKind::AcidPool,
                    EffectKind::DebrisShadow,
                    EffectKind::DebrisImpact,
                    EffectKind::QuicksilverWave,
                    EffectKind::MercuryPool,
                    EffectKind::MadnessSpray,
                    EffectKind::Wisp,
                    EffectKind::AmalgamationBlast,
                ]));
            }
        }
        paths
    }
}

/// The sheet path of every effect in `kinds`.
fn effect_paths(kinds: &[EffectKind]) -> Vec<String> {
    kinds.iter().map(|kind| kind.sheet_path()).collect()
}

/// The five clips every player look has. Only the farmer adds the farm actions,
/// because the other packs do not ship `water`/`plant` art.
const PLAYER_CLIPS: [PlayerAnimState; 5] = [
    PlayerAnimState::Idle,
    PlayerAnimState::Walk,
    PlayerAnimState::LightAttack,
    PlayerAnimState::HeavyAttack,
    PlayerAnimState::Death,
];

const FARMER_CLIPS: [PlayerAnimState; 2] = [PlayerAnimState::Water, PlayerAnimState::Plant];

/// Every sheet a player look needs.
pub fn player_paths(look: PlayerLook) -> Vec<String> {
    let mut count = PLAYER_CLIPS.len();
    if look == PlayerLook::Farmer {
        count += FARMER_CLIPS.len();
    }

    let mut paths = Vec::with_capacity(count * Facing8::ALL.len());
    for state in PLAYER_CLIPS {
        push_facings(&mut paths, state, look);
    }
    if look == PlayerLook::Farmer {
        for state in FARMER_CLIPS {
            push_facings(&mut paths, state, look);
        }
    }
    paths
}

fn push_facings(paths: &mut Vec<String>, state: PlayerAnimState, look: PlayerLook) {
    for facing in Facing8::ALL {
        paths.push(state.sheet_path(look, facing));
    }
}

/// Every sheet a boss pack needs.
pub fn boss_paths(pack: BossPack) -> Vec<String> {
    let attacks: &[BossAnimState] = match pack {
        BossPack::Excavator => &[
            BossAnimState::TailingsSurge,
            BossAnimState::ExcavatorSlam,
            BossAnimState::DebrisRain,
        ],
        BossPack::Mercuril => &[
            BossAnimState::MirrorStep,
            BossAnimState::QuicksilverWave,
            BossAnimState::Madness,
        ],
    };

    let mut paths = Vec::with_capacity((2 + attacks.len() + 1) * Facing8::ALL.len());
    for state in [BossAnimState::Idle, BossAnimState::Walk] {
        push_boss_facings(&mut paths, state, pack);
    }
    for state in attacks {
        push_boss_facings(&mut paths, *state, pack);
    }
    push_boss_facings(&mut paths, BossAnimState::Death, pack);
    paths
}

fn push_boss_facings(paths: &mut Vec<String>, state: BossAnimState, pack: BossPack) {
    for facing in Facing8::ALL {
        paths.push(state.sheet_path(pack, facing));
    }
}

/// The strong handles for the scene currently resident.
///
/// Rebuilding reuses handles for paths both the old and new scene share and
/// drops the rest, so switching scenes unloads whatever the new scene does not
/// need without ever reloading a sheet that is staying.
#[derive(Resource, Default)]
pub struct SceneAssetManifest {
    resident: HashMap<String, Handle<Image>>,
    target: Option<LoadTarget>,
}

impl SceneAssetManifest {
    /// Makes `target`'s sheets resident, reusing any already loaded.
    pub fn begin(&mut self, target: LoadTarget, paths: Vec<String>, server: &AssetServer) {
        let mut next = HashMap::with_capacity(paths.len());
        for path in paths {
            let handle = self
                .resident
                .remove(&path)
                .unwrap_or_else(|| server.load(path.clone()));
            next.insert(path, handle);
        }
        // Whatever is left in `resident` belonged only to the old scene; the
        // assignment drops those handles.
        self.resident = next;
        self.target = Some(target);
    }

    pub fn target(&self) -> Option<LoadTarget> {
        self.target
    }

    pub fn total(&self) -> usize {
        self.resident.len()
    }

    /// How many sheets have settled (loaded or failed).
    pub fn loaded(&self, server: &AssetServer) -> usize {
        self.resident
            .values()
            .filter(|handle| is_terminal(server, handle))
            .count()
    }

    /// Fraction of sheets that have settled, `0.0` when nothing is queued.
    pub fn progress(&self, server: &AssetServer) -> f32 {
        match self.total() {
            0 => 0.0,
            total => self.loaded(server) as f32 / total as f32,
        }
    }

    /// True once every sheet has settled. A missing server (headless tests)
    /// means there is nothing to wait for.
    pub fn is_complete(&self, server: Option<&AssetServer>) -> bool {
        let Some(server) = server else {
            return true;
        };
        self.resident
            .values()
            .all(|handle| is_terminal(server, handle))
    }

    /// Releases every handle so the assets can be unloaded.
    pub fn clear(&mut self) {
        self.resident.clear();
        self.target = None;
    }
}

fn is_terminal(server: &AssetServer, handle: &Handle<Image>) -> bool {
    matches!(
        server.get_load_state(handle.id()),
        Some(LoadState::Loaded | LoadState::Failed(_))
    )
}

/// Why the game is loading and where play should resume once it finishes.
#[derive(Resource, Default)]
pub struct LoadingContext {
    pub target: Option<LoadTarget>,
    pub resume_phase: Option<DayPhase>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_farm_loads_only_the_farmer() {
        let paths = LoadTarget::Farm.required_paths(Some(GearSet::Master));
        assert_eq!(paths.len(), 7 * Facing8::ALL.len());
        assert!(
            paths
                .iter()
                .all(|path| path.starts_with("sprite_packs/Farmer-spritesheet/"))
        );
    }

    #[test]
    fn arenas_load_the_worn_armor_look_and_one_boss() {
        let paths = LoadTarget::ArenaA.required_paths(Some(GearSet::BossA));
        assert_eq!(paths.len(), (5 + 6) * Facing8::ALL.len() + 5);
        assert!(
            paths
                .iter()
                .any(|path| path.starts_with("sprite_packs/Ember-Gear-spritesheet/")),
            "arena A should preload the ember look for ember armor"
        );
        assert!(
            paths
                .iter()
                .any(|path| path.starts_with("sprite_packs/Excavator-spritesheet/"))
        );
        assert!(
            !paths
                .iter()
                .any(|path| path.starts_with("sprite_packs/Mercuril-spritesheet/")),
            "arena A should not carry the other boss"
        );
        assert!(
            paths
                .iter()
                .any(|path| path.starts_with("sprite_packs/Effects/Tailings Surge/")),
            "arena A should preload the excavator's effects"
        );
    }

    #[test]
    fn arena_b_loads_the_other_boss() {
        let paths = LoadTarget::ArenaB.required_paths(Some(GearSet::BossB));
        assert_eq!(paths.len(), (5 + 6) * Facing8::ALL.len() + 4);
        assert!(
            paths
                .iter()
                .any(|path| path.starts_with("sprite_packs/Mercuril-spritesheet/"))
        );
        assert!(
            !paths
                .iter()
                .any(|path| path.starts_with("sprite_packs/Excavator-spritesheet/"))
        );
        assert!(
            paths
                .iter()
                .any(|path| path.starts_with("sprite_packs/Effects/Quicksilver Wave/"))
        );
    }

    #[test]
    fn the_dual_arena_loads_both_bosses() {
        let paths = LoadTarget::ArenaDual.required_paths(Some(GearSet::BossB));
        assert_eq!(paths.len(), (5 + 6 + 6) * Facing8::ALL.len() + 10);
        assert!(
            paths
                .iter()
                .any(|path| path.starts_with("sprite_packs/Excavator-spritesheet/"))
        );
        assert!(
            paths
                .iter()
                .any(|path| path.starts_with("sprite_packs/Mercuril-spritesheet/"))
        );
        assert!(
            paths
                .iter()
                .any(|path| path.starts_with("sprite_packs/Effects/Amalgamation Blast/")),
            "the finale needs the amalgamation effect"
        );
    }

    #[test]
    fn boss_select_preloads_the_farmer_and_one_idle_frame_per_boss() {
        let paths = LoadTarget::BossSelect.required_paths(Some(GearSet::Master));
        assert_eq!(paths.len(), 7 * Facing8::ALL.len() + 2);
        assert!(
            paths
                .iter()
                .any(|path| path.starts_with("sprite_packs/Farmer-spritesheet/"))
        );
        assert!(
            paths.iter().any(|path| path
                == "sprite_packs/Excavator-spritesheet/iso_idle_down_right/spritesheet.png")
        );
        assert!(
            paths.iter().any(|path| path
                == "sprite_packs/Mercuril-spritesheet/iso_idle_down_right/spritesheet.png")
        );
        // The full packs load on arena entry, not for the menu.
        assert!(!paths.iter().any(|path| path.contains("Excavator Slam")));
        assert!(!paths.iter().any(|path| path.contains("Madness")));
    }

    #[test]
    fn only_the_farmer_gets_the_farm_action_clips() {
        for look in [
            PlayerLook::Starter,
            PlayerLook::BossA,
            PlayerLook::BossB,
            PlayerLook::Master,
        ] {
            let paths = player_paths(look);
            assert!(
                !paths.iter().any(|path| path.contains("/water ")),
                "{look:?} has no water clip"
            );
            assert!(
                !paths.iter().any(|path| path.contains("/plant ")),
                "{look:?} has no plant clip"
            );
        }
        let farmer = player_paths(PlayerLook::Farmer);
        assert!(farmer.iter().any(|path| path.contains("/water ")));
        assert!(farmer.iter().any(|path| path.contains("/plant ")));
    }

    #[test]
    fn required_paths_are_unique() {
        for target in [
            LoadTarget::Farm,
            LoadTarget::BossSelect,
            LoadTarget::ArenaA,
            LoadTarget::ArenaB,
            LoadTarget::ArenaDual,
        ] {
            let paths = target.required_paths(Some(GearSet::BossA));
            let mut unique = paths.clone();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), paths.len(), "{target:?} has duplicate paths");
        }
    }

    #[test]
    fn every_target_maps_to_a_level() {
        assert_eq!(LoadTarget::Farm.level(), LevelId::Farm);
        assert_eq!(LoadTarget::BossSelect.level(), LevelId::Farm);
        assert_eq!(LoadTarget::ArenaA.level(), LevelId::ArenaA);
        assert_eq!(LoadTarget::ArenaB.level(), LevelId::ArenaB);
        assert_eq!(LoadTarget::ArenaDual.level(), LevelId::ArenaDual);
    }

    #[test]
    fn boss_targets_map_from_boss_ids() {
        assert_eq!(LoadTarget::for_boss(BossId::BossA), LoadTarget::ArenaA);
        assert_eq!(LoadTarget::for_boss(BossId::BossB), LoadTarget::ArenaB);
        assert_eq!(LoadTarget::for_boss(BossId::Dual), LoadTarget::ArenaDual);
    }

    #[test]
    fn an_empty_manifest_is_complete_without_a_server() {
        let manifest = SceneAssetManifest::default();
        assert!(manifest.is_complete(None));
        assert_eq!(manifest.total(), 0);
        assert!(manifest.target().is_none());
    }
}
