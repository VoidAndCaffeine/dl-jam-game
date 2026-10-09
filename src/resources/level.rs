use crate::levels::grid::SolidGrid;
use crate::levels::{LevelId, PropKind};
use crate::utils::level_parse::LevelDef;
use bevy::prelude::*;
use bevy::reflect::Reflect;

/// The level the world is currently showing.
#[derive(Resource, Reflect, Clone, Debug, Default)]
pub struct ActiveLevel {
    pub id: LevelId,
    pub def: LevelDef,
}

/// Marks every entity spawned as part of a level, so switching rooms only
/// despawns what belongs to the room being left.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelEntity {
    pub level: LevelId,
}

impl LevelEntity {
    pub fn of(level: LevelId) -> Self {
        Self { level }
    }
}

/// Where the player should appear, taken from the level's `^` marker.
#[derive(Resource, Reflect, Clone, Copy, Debug, Default)]
pub struct PlayerSpawn {
    pub position: Vec2,
}

/// Where the boss should appear, taken from a level's `b` marker(s).
///
/// Single-boss arenas have one marker; the dual arena has two, one per half.
#[derive(Resource, Reflect, Clone, Debug, Default)]
pub struct BossSpawn {
    /// The first marker, kept for single-boss fights and existing callers.
    pub position: Vec2,
    /// Every marker, in file order.
    pub positions: Vec<Vec2>,
}

impl BossSpawn {
    /// Every spawn, falling back to the single position so older callers and
    /// hand-built test levels still work.
    pub fn all(&self) -> Vec<Vec2> {
        if self.positions.is_empty() && self.position != Vec2::ZERO {
            vec![self.position]
        } else {
            self.positions.clone()
        }
    }
}

/// Set this to move between rooms. [`apply_level_request`] does the work.
#[derive(Resource, Reflect, Clone, Copy, Debug, Default)]
pub struct LevelRequest(pub Option<LevelId>);

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum LevelSet {
    Load,
    TileChunk,
}

/// World position of the first prop of `kind`, if the level has one.
pub fn prop_position(grid: &SolidGrid, def: &LevelDef, kind: PropKind) -> Option<Vec2> {
    let (col, file_row) = def.prop(kind)?;
    Some(grid.prop_center(def, col, file_row))
}

/// World positions of every prop of `kind`, in file order.
pub fn prop_positions(grid: &SolidGrid, def: &LevelDef, kind: PropKind) -> Vec<Vec2> {
    def.props_of(kind)
        .into_iter()
        .map(|(col, file_row)| grid.prop_center(def, col, file_row))
        .collect()
}

/// Parses a level and derives its collision grid, panicking on a broken file.
///
/// Level files are compiled in, so a malformed one is a build-time mistake
/// rather than something a player can trigger.
pub fn build_level(id: LevelId) -> (LevelDef, SolidGrid) {
    let def = id
        .parse()
        .unwrap_or_else(|error| panic!("level {} is invalid: {error}", id.file_name()));
    let grid = SolidGrid::from_level(&def);
    (def, grid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_farm_places_its_props_where_the_file_says() {
        let (def, grid) = build_level(LevelId::Farm);
        assert_eq!(def.width, 40);

        let spawn = prop_position(&grid, &def, PropKind::PlayerSpawn).unwrap();
        let pots = prop_positions(&grid, &def, PropKind::Pot);
        let station = prop_position(&grid, &def, PropKind::CraftingStation).unwrap();
        let gate = prop_position(&grid, &def, PropKind::ArenaGate).unwrap();

        assert_eq!(pots.len(), 9);
        assert!(!grid.is_solid_at(spawn));
        assert!(spawn.x > station.x, "player starts right of the station");
        assert!(station.x < pots[0].x, "pots sit right of the station");
        assert!(pots[8].x > gate.x, "pots are to the right of the gate");
        assert!(
            (spawn.y - pots[0].y).abs() > 1.0,
            "pots should not sit on the spawn tile"
        );
    }

    #[test]
    fn the_pots_form_a_grid_evenly_spaced_in_world_space() {
        let (def, grid) = build_level(LevelId::Farm);
        let pots = prop_positions(&grid, &def, PropKind::Pot);
        let column_gap = pots[1].x - pots[0].x;
        let row_gap = pots[0].y - pots[3].y;
        assert_eq!(column_gap, 3.0 * grid.tile_size());
        assert_eq!(row_gap, 3.0 * grid.tile_size(), "rows go down the file");
        assert_eq!(pots[3].x, pots[0].x, "same column");
        assert_eq!(pots[4].x, pots[1].x, "next row keeps the column grid");
        assert_eq!(pots[4].y, pots[3].y, "same row");
    }

    #[test]
    fn arenas_have_a_boss_and_a_player_but_no_pots() {
        for id in [LevelId::ArenaA, LevelId::ArenaB, LevelId::ArenaDual] {
            let (def, grid) = build_level(id);
            assert!(prop_position(&grid, &def, PropKind::PlayerSpawn).is_some());
            assert!(prop_position(&grid, &def, PropKind::BossSpawn).is_some());
            assert!(prop_positions(&grid, &def, PropKind::Pot).is_empty());
        }
    }

    #[test]
    fn the_player_and_boss_facing_sides_differ_per_arena() {
        let (_, arena_a) = build_level(LevelId::ArenaA);
        let (_, arena_b) = build_level(LevelId::ArenaB);
        let player_a = prop_position(
            &arena_a,
            &LevelId::ArenaA.parse().unwrap(),
            PropKind::PlayerSpawn,
        )
        .unwrap();
        let boss_a = prop_position(
            &arena_a,
            &LevelId::ArenaA.parse().unwrap(),
            PropKind::BossSpawn,
        )
        .unwrap();
        assert!(
            (player_a.x - boss_a.x).abs() < 1.0,
            "arena A should start the player under the boss"
        );

        let def_b = LevelId::ArenaB.parse().unwrap();
        let player_b = prop_position(&arena_b, &def_b, PropKind::PlayerSpawn).unwrap();
        let boss_b = prop_position(&arena_b, &def_b, PropKind::BossSpawn).unwrap();
        assert!(player_b.x < boss_b.x, "arena B should be a diagonal fight");
    }

    #[test]
    fn an_unknown_prop_has_no_position() {
        let (def, grid) = build_level(LevelId::ArenaA);
        assert_eq!(prop_position(&grid, &def, PropKind::CraftingStation), None);
    }

    #[test]
    fn every_arena_boss_spawns_clear_of_the_walls() {
        use crate::components::boss::BossId;
        use crate::constants::boss_size;

        // A boss collides with its whole sprite box, so the spawn has to clear
        // the walls for that box, not just the tile it stands on.
        let cases = [
            (LevelId::ArenaA, vec![boss_size(BossId::BossA)]),
            (LevelId::ArenaB, vec![boss_size(BossId::BossB)]),
            (
                LevelId::ArenaDual,
                vec![boss_size(BossId::BossA), boss_size(BossId::BossB)],
            ),
        ];
        for (id, sizes) in cases {
            let (def, grid) = build_level(id);
            let spawns = prop_positions(&grid, &def, PropKind::BossSpawn);
            assert_eq!(spawns.len(), sizes.len(), "{}", id.file_name());
            for (position, size) in spawns.iter().zip(sizes) {
                assert!(
                    !grid.aabb_hits(*position, Vec2::splat(size * 0.5), &[]),
                    "{}: a boss spawns stuck in a wall at {position:?}",
                    id.file_name()
                );
            }
        }
    }

    #[test]
    fn level_entities_remember_their_room() {
        assert_eq!(LevelEntity::of(LevelId::Farm).level, LevelId::Farm);
    }
}
