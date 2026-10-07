pub mod grid;
pub mod legend;

pub use grid::SolidGrid;
pub use legend::{PropKind, TileKind};

use crate::utils::level_parse::{LevelDef, LevelParseError};

#[derive(bevy::reflect::Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LevelId {
    #[default]
    Farm,
    ArenaA,
    ArenaB,
    ArenaDual,
}

impl LevelId {
    pub const ALL: [Self; 4] = [Self::Farm, Self::ArenaA, Self::ArenaB, Self::ArenaDual];

    pub fn source(self) -> &'static str {
        match self {
            Self::Farm => include_str!("../../levels/farm.txt"),
            Self::ArenaA => include_str!("../../levels/arena_a.txt"),
            Self::ArenaB => include_str!("../../levels/arena_b.txt"),
            Self::ArenaDual => include_str!("../../levels/arena_dual.txt"),
        }
    }

    pub fn file_name(self) -> &'static str {
        match self {
            Self::Farm => "farm.txt",
            Self::ArenaA => "arena_a.txt",
            Self::ArenaB => "arena_b.txt",
            Self::ArenaDual => "arena_dual.txt",
        }
    }

    pub fn parse(self) -> Result<LevelDef, LevelParseError> {
        crate::utils::level_parse::parse(self.source())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels() -> Vec<(LevelId, LevelDef)> {
        LevelId::ALL
            .iter()
            .map(|id| (*id, id.parse().expect("level file parses")))
            .collect()
    }

    #[test]
    fn every_level_file_parses() {
        for (id, level) in levels() {
            assert!(level.width > 0, "{} has no width", id.file_name());
            assert!(level.height > 0, "{} has no height", id.file_name());
            assert_eq!(
                level.tiles.len(),
                (level.width * level.height) as usize,
                "{} tile count",
                id.file_name()
            );
        }
    }

    #[test]
    fn every_level_uses_the_default_tile_size() {
        for (id, level) in levels() {
            assert_eq!(level.tile_size, 32, "{}", id.file_name());
        }
    }

    #[test]
    fn every_level_has_exactly_one_player_spawn() {
        for (id, level) in levels() {
            assert!(
                level.requires_single(PropKind::PlayerSpawn).is_ok(),
                "{} needs one ^",
                id.file_name()
            );
        }
    }

    #[test]
    fn the_farm_has_nine_pots_a_station_and_a_gate() {
        let level = LevelId::Farm.parse().unwrap();
        assert_eq!(level.props_of(PropKind::Pot).len(), 9);
        assert!(level.requires_single(PropKind::CraftingStation).is_ok());
        assert!(level.requires_single(PropKind::ArenaGate).is_ok());
        assert_eq!(level.props_of(PropKind::BossSpawn), Vec::new());
    }

    #[test]
    fn the_farm_pots_are_a_three_by_three_grid() {
        let level = LevelId::Farm.parse().unwrap();
        let pots = level.props_of(PropKind::Pot);
        assert_eq!(pots.len(), 9);

        let mut columns: Vec<u32> = pots.iter().map(|(x, _)| *x).collect();
        let mut rows: Vec<u32> = pots.iter().map(|(_, y)| *y).collect();
        columns.sort_unstable();
        columns.dedup();
        rows.sort_unstable();
        rows.dedup();
        assert_eq!(columns.len(), 3, "pots should sit on 3 columns");
        assert_eq!(rows.len(), 3, "pots should sit on 3 rows");

        for set in [&columns, &rows] {
            let gap = set[1] - set[0];
            assert!(gap >= 2, "pots need breathing room to be clickable");
            for index in 1..set.len() {
                assert_eq!(set[index] - set[index - 1], gap, "uneven spacing");
            }
        }
    }

    #[test]
    fn arenas_have_a_boss_spawn_and_no_farming_props() {
        for id in [LevelId::ArenaA, LevelId::ArenaB] {
            let level = id.parse().unwrap();
            assert!(
                level.requires_single(PropKind::BossSpawn).is_ok(),
                "{} needs one b",
                id.file_name()
            );
            assert!(
                level.props_of(PropKind::Pot).is_empty(),
                "{} should not have pots",
                id.file_name()
            );
            assert!(
                level.requires_single(PropKind::CraftingStation).is_err(),
                "{} should not have a station",
                id.file_name()
            );
        }
    }

    #[test]
    fn the_dual_arena_has_two_boss_spawns_one_for_each_half() {
        let level = LevelId::ArenaDual.parse().unwrap();
        assert_eq!(level.props_of(PropKind::BossSpawn).len(), 2);
        let (a, b) = {
            let spawns = level.props_of(PropKind::BossSpawn);
            (spawns[0], spawns[1])
        };
        assert_ne!(a, b, "the two halves need distinct spots");
    }

    #[test]
    fn arenas_are_roomy_enough_to_fight_in() {
        for id in [LevelId::ArenaA, LevelId::ArenaB, LevelId::ArenaDual] {
            let level = id.parse().unwrap();
            assert!(
                level.width >= 30 && level.height >= 18,
                "{}",
                id.file_name()
            );
        }
    }

    #[test]
    fn nothing_spawns_inside_a_wall() {
        for (id, level) in levels() {
            for prop in &level.props {
                assert!(
                    !level.is_solid(prop.x, prop.y),
                    "{}: {:?} at {},{} is inside a wall",
                    id.file_name(),
                    prop.kind,
                    prop.x,
                    prop.y
                );
            }
        }
    }

    #[test]
    fn every_room_is_sealed_by_a_wall_border() {
        for (id, level) in levels() {
            for x in 0..level.width {
                assert!(
                    level.is_solid(x, 0),
                    "{}: top edge at x={x}",
                    id.file_name()
                );
                assert!(
                    level.is_solid(x, level.height - 1),
                    "{}: bottom edge at x={x}",
                    id.file_name()
                );
            }
            for y in 0..level.height {
                assert!(
                    level.is_solid(0, y),
                    "{}: left edge at y={y}",
                    id.file_name()
                );
                assert!(
                    level.is_solid(level.width - 1, y),
                    "{}: right edge at y={y}",
                    id.file_name()
                );
            }
        }
    }

    #[test]
    fn rooms_have_open_floor_to_walk_on() {
        for (id, level) in levels() {
            let open = level
                .tiles
                .iter()
                .filter(|kind| !kind.is_solid() && **kind != TileKind::Void)
                .count();
            let total = (level.width * level.height) as usize;
            assert!(open * 2 > total, "{} is too cramped", id.file_name());
        }
    }

    #[test]
    fn the_farm_pond_does_not_block_the_path_to_the_pots() {
        let level = LevelId::Farm.parse().unwrap();
        for x in 1..level.width - 1 {
            assert!(!level.is_solid(x, 19), "the main path is blocked at x={x}");
        }
    }

    #[test]
    fn level_ids_expose_distinct_sources() {
        let sources: Vec<&str> = LevelId::ALL.iter().map(|id| id.source()).collect();
        for (index, source) in sources.iter().enumerate() {
            assert!(!source.trim().is_empty());
            for other in &sources[index + 1..] {
                assert_ne!(source, other);
            }
        }
    }
}
