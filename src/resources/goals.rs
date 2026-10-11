//! The run's objective checklist.
//!
//! Goals are grouped into ordered tiers. Only one tier is active at a time:
//! finishing every goal in a tier silently unlocks the next, which is what the
//! `-->` in the design notes describes. The HUD shows the active tier's
//! outstanding goals; the pause overlay shows every *revealed* tier (the
//! completed ones plus the active one) so later tiers are never spoiled.

use crate::components::boss::BossId;
use crate::components::pot::CropType;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// One checkable objective. Stable across saves.
#[derive(Reflect, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum GoalId {
    // Tier 1
    PlantStarter,
    WaterCrop,
    // Tier 2
    HarvestStarter,
    CraftFullSet,
    BeatExcavator,
    // Tier 3
    HarvestCinderCap,
    BeatMercurial,
    // Tier 4
    CraftDreamLayerSet,
    BeatDualBoss,
}

impl GoalId {
    pub const ALL: [GoalId; 9] = [
        GoalId::PlantStarter,
        GoalId::WaterCrop,
        GoalId::HarvestStarter,
        GoalId::CraftFullSet,
        GoalId::BeatExcavator,
        GoalId::HarvestCinderCap,
        GoalId::BeatMercurial,
        GoalId::CraftDreamLayerSet,
        GoalId::BeatDualBoss,
    ];
}

/// A goal's static definition: its id and the text the HUD/pause list shows.
pub struct GoalDef {
    pub id: GoalId,
    pub description: &'static str,
}

/// A group of goals gated behind the previous group's completion.
pub struct GoalTierDef {
    pub title: &'static str,
    pub goals: &'static [GoalDef],
}

/// The whole checklist, in unlock order. The first tier is active on a new run.
pub const GOAL_TIERS: &[GoalTierDef] = &[
    GoalTierDef {
        title: "First Sprout",
        goals: &[
            GoalDef {
                id: GoalId::PlantStarter,
                description: "Plant a Quicksilver Reed",
            },
            GoalDef {
                id: GoalId::WaterCrop,
                description: "Water a Crop",
            },
        ],
    },
    GoalTierDef {
        title: "Tools of the Trade",
        goals: &[
            GoalDef {
                id: GoalId::HarvestStarter,
                description: "Harvest a Quicksilver Reed",
            },
            GoalDef {
                id: GoalId::CraftFullSet,
                description: "Craft a Full Gear Set",
            },
            GoalDef {
                id: GoalId::BeatExcavator,
                description: "Beat The Excavator",
            },
        ],
    },
    GoalTierDef {
        title: "Beyond the Farm",
        goals: &[
            GoalDef {
                id: GoalId::HarvestCinderCap,
                description: "Harvest a Cinder Cap",
            },
            GoalDef {
                id: GoalId::BeatMercurial,
                description: "Beat Mercurial",
            },
        ],
    },
    GoalTierDef {
        title: "DreamLayer Mastery",
        goals: &[
            GoalDef {
                id: GoalId::CraftDreamLayerSet,
                description: "Craft the Full DreamLayer Set",
            },
            GoalDef {
                id: GoalId::BeatDualBoss,
                description: "Beat the Dual Bossfight",
            },
        ],
    },
];

pub const TIER_COUNT: usize = GOAL_TIERS.len();

/// The description for one goal.
pub fn description_for(id: GoalId) -> &'static str {
    GOAL_TIERS
        .iter()
        .flat_map(|tier| tier.goals)
        .find(|goal| goal.id == id)
        .map(|goal| goal.description)
        .unwrap_or("")
}

/// How far the run has come through the checklist.
///
/// `completed` is the set of finished goals; `tier` is the index of the active
/// (highest revealed) tier. Both survive a save.
#[derive(Resource, Reflect, Serialize, Deserialize, Default, Debug, Clone)]
pub struct GoalsProgress {
    pub completed: Vec<GoalId>,
    pub tier: usize,
}

impl GoalsProgress {
    pub fn is_completed(&self, id: GoalId) -> bool {
        self.completed.contains(&id)
    }

    /// Marks a goal done. Returns whether this call actually changed anything.
    pub fn complete(&mut self, id: GoalId) -> bool {
        if self.is_completed(id) {
            return false;
        }
        self.completed.push(id);
        true
    }

    /// The active tier index, clamped so a finished checklist stays on the last
    /// tier instead of running off the end.
    pub fn active_tier(&self) -> usize {
        self.tier.min(TIER_COUNT.saturating_sub(1))
    }

    pub fn current_tier(&self) -> &'static GoalTierDef {
        &GOAL_TIERS[self.active_tier()]
    }

    /// The active tier's goals still outstanding, in tier order.
    pub fn active_goals(&self) -> Vec<GoalId> {
        self.current_tier()
            .goals
            .iter()
            .map(|goal| goal.id)
            .filter(|id| !self.is_completed(*id))
            .collect()
    }

    /// Whether every goal in `tier` is done.
    pub fn is_tier_completed(&self, tier: usize) -> bool {
        GOAL_TIERS
            .get(tier)
            .is_some_and(|def| def.goals.iter().all(|goal| self.is_completed(goal.id)))
    }

    /// Advances past every fully-complete tier. Silent by design: callers only
    /// see the next tier's goals appear.
    pub fn advance(&mut self) {
        while self.tier < TIER_COUNT.saturating_sub(1) && self.is_tier_completed(self.tier) {
            self.tier += 1;
        }
    }
}

/// The goal a planted crop satisfies, if any.
pub fn goal_for_plant(crop: CropType) -> Option<GoalId> {
    match crop {
        CropType::Starter => Some(GoalId::PlantStarter),
        CropType::CropA | CropType::CropB => None,
    }
}

/// The goal a harvested crop satisfies, if any.
pub fn goal_for_harvest(crop: CropType) -> Option<GoalId> {
    match crop {
        CropType::Starter => Some(GoalId::HarvestStarter),
        CropType::CropA => Some(GoalId::HarvestCinderCap),
        CropType::CropB => None,
    }
}

/// The goal a defeated boss satisfies, if any.
pub fn goal_for_boss(id: BossId) -> Option<GoalId> {
    match id {
        BossId::BossA => Some(GoalId::BeatExcavator),
        BossId::BossB => Some(GoalId::BeatMercurial),
        BossId::Dual => Some(GoalId::BeatDualBoss),
    }
}

/// Reserved, non-gating objectives. Not wired in yet: the HUD tracks only the
/// main tier chain. Kept here so the shape is ready to switch on later.
#[allow(dead_code)]
pub mod side_objectives {
    /// A side objective's identity, separate from the main [`super::GoalId`]
    /// chain so the two can never be confused.
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub enum SideObjectiveId {
        WaterCrop,
        OpenForge,
        HitlessBoss,
        ReachDay10,
        UnlockAllCrops,
        CraftEveryPiece,
    }

    pub struct SideObjective {
        pub id: SideObjectiveId,
        pub description: &'static str,
    }

    pub const SIDE_OBJECTIVES: &[SideObjective] = &[
        SideObjective {
            id: SideObjectiveId::WaterCrop,
            description: "Water a crop",
        },
        SideObjective {
            id: SideObjectiveId::OpenForge,
            description: "Open the Forge",
        },
        SideObjective {
            id: SideObjectiveId::HitlessBoss,
            description: "Beat a boss without taking a hit",
        },
        SideObjective {
            id: SideObjectiveId::ReachDay10,
            description: "Reach Day 10",
        },
        SideObjective {
            id: SideObjectiveId::UnlockAllCrops,
            description: "Unlock every crop",
        },
        SideObjective {
            id: SideObjectiveId::CraftEveryPiece,
            description: "Craft every gear piece",
        },
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_run_starts_on_the_first_tier_with_nothing_done() {
        let goals = GoalsProgress::default();
        assert_eq!(goals.active_tier(), 0);
        assert!(goals.completed.is_empty());
        assert_eq!(
            goals.active_goals(),
            vec![GoalId::PlantStarter, GoalId::WaterCrop]
        );
    }

    #[test]
    fn completing_tier_one_unlocks_tier_two() {
        let mut goals = GoalsProgress::default();
        assert!(goals.complete(GoalId::PlantStarter));
        assert!(!goals.complete(GoalId::PlantStarter), "idempotent");
        assert_eq!(goals.active_tier(), 0, "watering is still outstanding");
        goals.complete(GoalId::WaterCrop);
        goals.advance();

        assert_eq!(goals.active_tier(), 1);
        assert_eq!(
            goals.active_goals(),
            vec![
                GoalId::HarvestStarter,
                GoalId::CraftFullSet,
                GoalId::BeatExcavator
            ]
        );
    }

    #[test]
    fn a_partly_done_tier_does_not_advance() {
        let mut goals = GoalsProgress {
            tier: 1,
            ..Default::default()
        };
        goals.complete(GoalId::HarvestStarter);
        goals.advance();

        assert_eq!(goals.active_tier(), 1, "still two goals to go");
        assert_eq!(
            goals.active_goals(),
            vec![GoalId::CraftFullSet, GoalId::BeatExcavator]
        );
    }

    #[test]
    fn finishing_a_tier_reveals_the_next() {
        let mut goals = GoalsProgress {
            tier: 1,
            ..Default::default()
        };
        for id in [
            GoalId::HarvestStarter,
            GoalId::CraftFullSet,
            GoalId::BeatExcavator,
        ] {
            goals.complete(id);
        }
        goals.advance();

        assert_eq!(goals.active_tier(), 2);
        assert_eq!(
            goals.active_goals(),
            vec![GoalId::HarvestCinderCap, GoalId::BeatMercurial]
        );
    }

    #[test]
    fn the_last_tier_stays_active_once_its_goals_are_done() {
        let mut goals = GoalsProgress {
            tier: TIER_COUNT - 1,
            ..Default::default()
        };
        for id in [GoalId::CraftDreamLayerSet, GoalId::BeatDualBoss] {
            goals.complete(id);
        }
        goals.advance();

        assert_eq!(
            goals.active_tier(),
            TIER_COUNT - 1,
            "never runs past the end"
        );
        assert!(goals.active_goals().is_empty());
        assert!(goals.is_tier_completed(TIER_COUNT - 1));
    }

    #[test]
    fn advance_skips_over_already_complete_tiers() {
        let mut goals = GoalsProgress::default();
        // Pretend the first two tiers were finished in one go.
        goals.complete(GoalId::PlantStarter);
        for id in [
            GoalId::HarvestStarter,
            GoalId::WaterCrop,
            GoalId::CraftFullSet,
            GoalId::BeatExcavator,
        ] {
            goals.complete(id);
        }
        goals.advance();
        assert_eq!(goals.active_tier(), 2);
    }

    #[test]
    fn every_goal_has_a_unique_description() {
        let mut seen: Vec<GoalId> = Vec::new();
        for tier in GOAL_TIERS {
            assert!(!tier.title.is_empty());
            for goal in tier.goals {
                assert!(!goal.description.is_empty());
                assert!(!seen.contains(&goal.id), "duplicate {:?}", goal.id);
                seen.push(goal.id);
            }
        }
        assert_eq!(seen.len(), GoalId::ALL.len());
    }

    #[test]
    fn event_mappings_cover_the_intended_sources() {
        assert_eq!(
            goal_for_plant(CropType::Starter),
            Some(GoalId::PlantStarter)
        );
        assert_eq!(goal_for_plant(CropType::CropA), None);
        assert_eq!(
            goal_for_harvest(CropType::Starter),
            Some(GoalId::HarvestStarter)
        );
        assert_eq!(
            goal_for_harvest(CropType::CropA),
            Some(GoalId::HarvestCinderCap)
        );
        assert_eq!(goal_for_harvest(CropType::CropB), None);
        assert_eq!(goal_for_boss(BossId::BossA), Some(GoalId::BeatExcavator));
        assert_eq!(goal_for_boss(BossId::BossB), Some(GoalId::BeatMercurial));
        assert_eq!(goal_for_boss(BossId::Dual), Some(GoalId::BeatDualBoss));
    }

    #[test]
    fn the_side_objective_skeleton_is_populated_and_unique() {
        use side_objectives::SIDE_OBJECTIVES;
        assert!(!SIDE_OBJECTIVES.is_empty());
        for (index, first) in SIDE_OBJECTIVES.iter().enumerate() {
            assert!(!first.description.is_empty());
            for second in SIDE_OBJECTIVES.iter().skip(index + 1) {
                assert_ne!(first.id, second.id);
            }
        }
    }
}
