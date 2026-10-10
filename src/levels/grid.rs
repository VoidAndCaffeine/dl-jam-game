use crate::levels::legend::TileKind;
use crate::utils::level_parse::LevelDef;
use bevy::math::UVec2;
use bevy::prelude::*;
use bevy::reflect::Reflect;

const MAX_SUB_STEPS: usize = 16;
const MAX_PUSH_OUT_STEPS: usize = 4;

/// A level's solid tiles in world order, plus the world-space mapping for them.
///
/// Row 0 is the **bottom** row of the room, matching world space and
/// `TilemapChunk::calculate_tile_transform`. Level files are written top-down,
/// so the flip happens once in [`SolidGrid::from_level`]; `level_tile_data`
/// applies the same flip for rendering.
/// Anything outside the grid counts as solid so nobody walks off the room.
#[derive(Resource, Reflect, Clone, Debug, PartialEq)]
pub struct SolidGrid {
    width: u32,
    height: u32,
    tile_size: f32,
    bottom_left: Vec2,
    solid: Vec<bool>,
}

impl Default for SolidGrid {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            tile_size: 32.0,
            bottom_left: Vec2::ZERO,
            solid: Vec::new(),
        }
    }
}

impl SolidGrid {
    /// Builds a grid for a level whose room is centred on the world origin.
    pub fn from_level(level: &LevelDef) -> Self {
        let size = Vec2::new(
            level.width as f32 * level.tile_size as f32,
            level.height as f32 * level.tile_size as f32,
        );
        Self::from_level_at(level, -size * 0.5)
    }

    /// Builds a grid whose bottom-left tile corner sits at `bottom_left`.
    pub fn from_level_at(level: &LevelDef, bottom_left: Vec2) -> Self {
        let width = level.width;
        let height = level.height;
        let mut solid = vec![false; (width * height) as usize];

        for file_row in 0..height {
            let world_row = level.world_row(file_row);
            for col in 0..width {
                let is_solid = level.is_solid(col, file_row);
                solid[(world_row * width + col) as usize] = is_solid;
            }
        }

        Self {
            width,
            height,
            tile_size: level.tile_size as f32,
            bottom_left,
            solid,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn tile_size(&self) -> f32 {
        self.tile_size
    }

    pub fn bottom_left(&self) -> Vec2 {
        self.bottom_left
    }

    pub fn world_size(&self) -> Vec2 {
        Vec2::new(
            self.width as f32 * self.tile_size,
            self.height as f32 * self.tile_size,
        )
    }

    pub fn center(&self) -> Vec2 {
        self.bottom_left + self.world_size() * 0.5
    }

    /// Confines a camera centre to the room, given how much of the world the
    /// camera can see. Rooms smaller than the view get pinned to their middle.
    pub fn clamp_to_room(&self, center: Vec2, visible_size: Vec2) -> Vec2 {
        let half = self.world_size() * 0.5;
        let limit = (half - visible_size * 0.5).max(Vec2::ZERO);
        let room_center = self.center();
        let offset = (center - room_center).clamp(-limit, limit);
        room_center + offset
    }

    /// World position of a tile's bottom-left corner.
    pub fn tile_min(&self, col: u32, row: u32) -> Vec2 {
        self.bottom_left + Vec2::new(col as f32 * self.tile_size, row as f32 * self.tile_size)
    }

    /// World position of a tile's centre.
    pub fn tile_center(&self, col: u32, row: u32) -> Vec2 {
        self.tile_min(col, row) + Vec2::splat(self.tile_size * 0.5)
    }

    /// World position for a prop written in a level file.
    pub fn prop_center(&self, level: &LevelDef, col: u32, file_row: u32) -> Vec2 {
        self.tile_center(col, level.world_row(file_row))
    }

    /// The tile containing `world`, or `None` when it is outside the room.
    pub fn world_to_tile(&self, world: Vec2) -> Option<UVec2> {
        let local = world - self.bottom_left;
        let col = (local.x / self.tile_size).floor();
        let row = (local.y / self.tile_size).floor();
        if col < 0.0 || row < 0.0 {
            return None;
        }
        let tile = UVec2::new(col as u32, row as u32);
        self.contains_tile(tile.x, tile.y).then_some(tile)
    }

    pub fn contains_tile(&self, col: u32, row: u32) -> bool {
        col < self.width && row < self.height
    }

    /// True when the tile blocks movement. Outside the room is solid too.
    pub fn is_solid(&self, col: u32, row: u32) -> bool {
        if !self.contains_tile(col, row) {
            return true;
        }
        self.solid[(row * self.width + col) as usize]
    }

    pub fn is_solid_at(&self, world: Vec2) -> bool {
        match self.world_to_tile(world) {
            Some(tile) => self.is_solid(tile.x, tile.y),
            None => true,
        }
    }

    pub fn solid_count(&self) -> usize {
        self.solid.iter().filter(|solid| **solid).count()
    }

    /// Whether a box centred on `center` overlaps any solid tile.
    pub fn aabb_hits_solid(&self, center: Vec2, half: Vec2) -> bool {
        self.aabb_hits(center, half, &[])
    }

    /// Whether a box overlaps a solid tile or one of the `obstacles`.
    pub fn aabb_hits(&self, center: Vec2, half: Vec2, obstacles: &[(Vec2, Vec2)]) -> bool {
        for (other_center, other_half) in obstacles {
            if aabbs_overlap(center, half, *other_center, *other_half) {
                return true;
            }
        }

        let min = center - half;
        let max = center + half;
        let (Some(first), Some(last)) = (self.world_to_tile(min), self.world_to_tile(max)) else {
            return true;
        };
        for row in first.y..=last.y {
            for col in first.x..=last.x {
                if self.is_solid(col, row) {
                    return true;
                }
            }
        }
        false
    }

    /// Moves a box by `delta`, sliding along walls and stopping at them.
    ///
    /// X is resolved before Y on each sub-step so corners slide instead of
    /// sticking. A blocked axis step is dropped whole, which leaves the box up
    /// to half a tile short of the wall - fine for a jam, and cheap.
    pub fn move_and_collide(&self, half: Vec2, from: Vec2, delta: Vec2) -> Vec2 {
        self.move_and_collide_with(&[], half, from, delta)
    }

    /// [`SolidGrid::move_and_collide`] against entity obstacles as well, given
    /// as `(centre, half_size)` pairs.
    pub fn move_and_collide_with(
        &self,
        obstacles: &[(Vec2, Vec2)],
        half: Vec2,
        from: Vec2,
        delta: Vec2,
    ) -> Vec2 {
        let max_step = (self.tile_size * 0.5).max(f32::EPSILON);
        let steps = ((delta.length() / max_step).ceil() as usize).clamp(1, MAX_SUB_STEPS);
        let step = delta / steps as f32;

        let mut position = from;
        for _ in 0..steps {
            let moved_x = Vec2::new(position.x + step.x, position.y);
            if !self.aabb_hits(moved_x, half, obstacles) {
                position = moved_x;
            }
            let moved_y = Vec2::new(position.x, position.y + step.y);
            if !self.aabb_hits(moved_y, half, obstacles) {
                position = moved_y;
            }
        }
        position
    }

    /// Pushes a box out of the entity obstacles it is already overlapping,
    /// along the axis with the smallest overlap.
    ///
    /// Solid tiles are deliberately not handled: a spawn inside a wall is a
    /// level authoring mistake, and `cargo test levels` already fails on it.
    pub fn push_out_of_obstacles(
        &self,
        obstacles: &[(Vec2, Vec2)],
        half: Vec2,
        from: Vec2,
    ) -> Vec2 {
        let mut position = from;
        for _ in 0..MAX_PUSH_OUT_STEPS {
            let Some((axis, resolved)) = smallest_overlap(half, position, obstacles) else {
                return position;
            };
            match axis {
                Axis::X => position.x = resolved,
                Axis::Y => position.y = resolved,
            }
        }
        position
    }
}

/// The cheapest way out of an overlap: which axis, and where it lands.
fn smallest_overlap(half: Vec2, from: Vec2, obstacles: &[(Vec2, Vec2)]) -> Option<(Axis, f32)> {
    let mut best: Option<(Axis, f32, f32)> = None;

    for (center, other_half) in obstacles {
        if !aabbs_overlap(from, half, *center, *other_half) {
            continue;
        }
        let sides = [
            (
                Axis::X,
                from.x + half.x - (center.x - other_half.x),
                center.x - other_half.x - half.x,
            ),
            (
                Axis::X,
                center.x + other_half.x - (from.x - half.x),
                center.x + other_half.x + half.x,
            ),
            (
                Axis::Y,
                from.y + half.y - (center.y - other_half.y),
                center.y - other_half.y - half.y,
            ),
            (
                Axis::Y,
                center.y + other_half.y - (from.y - half.y),
                center.y + other_half.y + half.y,
            ),
        ];
        for (axis, penetration, resolved) in sides {
            if penetration <= 0.0 {
                continue;
            }
            if best.is_none_or(|(_, smallest, _)| penetration < smallest) {
                best = Some((axis, penetration, resolved));
            }
        }
    }

    best.map(|(axis, _, resolved)| (axis, resolved))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

pub fn aabbs_overlap(a_center: Vec2, a_half: Vec2, b_center: Vec2, b_half: Vec2) -> bool {
    let separation = (a_center - b_center).abs();
    separation.x < a_half.x + b_half.x && separation.y < a_half.y + b_half.y
}

/// Builds a grid straight from a legend grid, for tests and tools.
pub fn grid_from_rows(rows: &[&str], tile_size: f32) -> SolidGrid {
    let height = rows.len() as u32;
    let width = rows.iter().map(|row| row.len()).max().unwrap_or(0) as u32;
    let tiles: Vec<TileKind> = rows
        .iter()
        .flat_map(|row| row.chars())
        .map(|c| TileKind::from_char(c).expect("known tile char"))
        .collect();

    let level = LevelDef {
        tile_size: tile_size as u32,
        width,
        height,
        tiles,
        props: Vec::new(),
    };
    SolidGrid::from_level(&level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels::LevelId;
    use crate::levels::legend::PropKind;

    const ROOM: &[&str] = &["#####", "#...#", "#.#.#", "#...#", "#####"];

    const HALF: Vec2 = Vec2::splat(8.0);

    const OPEN_ROOM: &[&str] = &[
        "#######", "#.....#", "#.....#", "#.....#", "#.....#", "#.....#", "#######",
    ];

    fn room() -> SolidGrid {
        grid_from_rows(ROOM, 32.0)
    }

    fn open_room() -> SolidGrid {
        grid_from_rows(OPEN_ROOM, 32.0)
    }

    #[test]
    fn a_centred_room_sits_on_the_world_origin() {
        let grid = room();
        assert_eq!(grid.width(), 5);
        assert_eq!(grid.height(), 5);
        assert_eq!(grid.tile_size(), 32.0);
        assert_eq!(grid.world_size(), Vec2::splat(160.0));
        assert_eq!(grid.center(), Vec2::ZERO);
        assert_eq!(grid.bottom_left(), Vec2::splat(-80.0));
    }

    #[test]
    fn tile_positions_start_at_the_bottom_left() {
        let grid = room();
        assert_eq!(grid.tile_min(0, 0), Vec2::splat(-80.0));
        assert_eq!(grid.tile_center(0, 0), Vec2::splat(-64.0));
        assert_eq!(grid.tile_min(4, 4), Vec2::new(48.0, 48.0));
        assert_eq!(grid.tile_center(4, 4), Vec2::new(64.0, 64.0));
    }

    #[test]
    fn world_and_tile_coordinates_round_trip() {
        let grid = room();
        for row in 0..grid.height() {
            for col in 0..grid.width() {
                let center = grid.tile_center(col, row);
                assert_eq!(
                    grid.world_to_tile(center),
                    Some(UVec2::new(col, row)),
                    "tile {col},{row} at {center}"
                );
            }
        }
    }

    #[test]
    fn positions_outside_the_room_have_no_tile() {
        let grid = room();
        assert_eq!(grid.world_to_tile(Vec2::new(-81.0, 0.0)), None);
        assert_eq!(grid.world_to_tile(Vec2::new(0.0, -81.0)), None);
        assert_eq!(grid.world_to_tile(Vec2::new(81.0, 0.0)), None);
        assert_eq!(grid.world_to_tile(Vec2::new(0.0, 81.0)), None);
        assert!(grid.is_solid_at(Vec2::new(500.0, 500.0)));
    }

    #[test]
    fn a_camera_cannot_look_past_the_room_edge() {
        let grid = room();
        let view = Vec2::splat(96.0);
        let far = grid.clamp_to_room(Vec2::new(10_000.0, 10_000.0), view);
        let half = grid.world_size() * 0.5;
        assert!(far.x + view.x * 0.5 <= half.x + 0.01, "right edge {far:?}");
        assert!(far.y + view.y * 0.5 <= half.y + 0.01, "top edge {far:?}");

        let inside = grid.clamp_to_room(Vec2::new(1.0, 2.0), view);
        assert_eq!(inside, Vec2::new(1.0, 2.0), "a centred view is untouched");
    }

    #[test]
    fn a_room_smaller_than_the_view_is_centred() {
        let grid = room();
        let centred = grid.clamp_to_room(Vec2::new(37.0, 11.0), Vec2::splat(1000.0));
        assert_eq!(centred, grid.center());
    }

    #[test]
    fn a_view_larger_than_the_room_still_shows_the_middle() {
        let grid = open_room();
        let clamped = grid.clamp_to_room(grid.center(), Vec2::splat(500.0));
        assert_eq!(clamped, grid.center());
    }

    #[test]
    fn the_wall_border_is_solid_and_the_floor_is_not() {
        let grid = room();
        assert!(grid.is_solid(0, 0));
        assert!(grid.is_solid(4, 4));
        assert!(grid.is_solid(0, 4));
        assert!(!grid.is_solid(1, 1));
        assert!(!grid.is_solid(3, 3));
        assert!(grid.is_solid_at(grid.tile_center(0, 0)));
        assert!(!grid.is_solid_at(grid.tile_center(2, 1)));
        assert!(!grid.is_solid_at(grid.tile_center(2, 3)));
    }

    #[test]
    fn the_room_centre_pillar_is_solid_in_world_order() {
        let grid = room();
        assert!(grid.is_solid(2, 2));
        assert!(grid.is_solid_at(Vec2::ZERO));
    }

    #[test]
    fn solid_tiles_are_counted_once() {
        let grid = room();
        let walls = ROOM
            .iter()
            .flat_map(|row| row.chars())
            .filter(|c| *c == '#')
            .count();
        assert_eq!(grid.solid_count(), walls);
    }

    #[test]
    fn the_file_is_flipped_into_world_order() {
        let grid = grid_from_rows(&["#.#", "..."], 32.0);
        // row 0 of the file is the top row, so it lands at the top in world terms
        assert!(grid.is_solid(0, 1));
        assert!(grid.is_solid(2, 1));
        assert!(!grid.is_solid(0, 0));
        assert!(!grid.is_solid(2, 0));
    }

    #[test]
    fn a_free_standing_box_does_not_hit_anything() {
        let grid = room();
        assert!(!grid.aabb_hits_solid(grid.tile_center(2, 1), HALF));
        assert!(!grid.aabb_hits_solid(grid.tile_center(1, 3), HALF));
    }

    #[test]
    fn a_box_overlapping_a_wall_hits_it() {
        let grid = room();
        assert!(grid.aabb_hits_solid(Vec2::new(48.0, 0.0), HALF));
        assert!(grid.aabb_hits_solid(grid.tile_center(2, 2), Vec2::splat(40.0)));
    }

    #[test]
    fn walking_into_a_wall_stops_short_of_it() {
        let grid = room();
        let start = grid.tile_center(1, 1);
        let moved = grid.move_and_collide(HALF, start, Vec2::new(100.0, 0.0));
        assert!(moved.x > start.x, "should move towards the wall");
        assert!(!grid.aabb_hits_solid(moved, HALF));
        assert!(
            moved.x < grid.tile_min(4, 1).x,
            "should stop before the wall at x={}, ended at {moved:?}",
            grid.tile_min(4, 1).x
        );
    }

    #[test]
    fn sliding_along_a_wall_keeps_moving() {
        let grid = room();
        let start = grid.tile_center(1, 1);
        let moved = grid.move_and_collide(HALF, start, Vec2::new(100.0, 100.0));
        assert!(moved.y > start.y, "should slide up past the wall");
        assert!(!grid.aabb_hits_solid(moved, HALF));
    }

    #[test]
    fn a_diagonal_step_into_a_corner_does_not_crash_or_tunnel() {
        let grid = room();
        let start = grid.tile_center(3, 1);
        let moved = grid.move_and_collide(HALF, start, Vec2::new(-100.0, 100.0));
        assert!(!grid.aabb_hits_solid(moved, HALF));
    }

    #[test]
    fn a_fast_step_cannot_tunnel_through_a_wall() {
        let grid = room();
        let start = grid.tile_center(1, 1);
        let moved = grid.move_and_collide(HALF, start, Vec2::new(1000.0, 0.0));
        assert!(
            moved.x < grid.tile_min(4, 1).x,
            "tunnelled through the wall to {moved:?}"
        );
        assert!(!grid.aabb_hits_solid(moved, HALF));
    }

    #[test]
    fn movement_outside_the_room_is_pulled_back_by_the_border() {
        let grid = room();
        let moved = grid.move_and_collide(HALF, grid.tile_center(2, 1), Vec2::new(0.0, 1000.0));
        assert!(!grid.aabb_hits_solid(moved, HALF));
        assert!(grid.contains_tile(
            grid.world_to_tile(moved).unwrap().x,
            grid.world_to_tile(moved).unwrap().y
        ));
    }

    #[test]
    fn standing_still_stays_put() {
        let grid = room();
        let start = grid.tile_center(2, 1);
        assert_eq!(grid.move_and_collide(HALF, start, Vec2::ZERO), start);
    }

    #[test]
    fn a_pot_blocks_the_player_like_a_wall() {
        let grid = room();
        let pot = grid.tile_center(3, 1);
        let obstacles = [(pot, Vec2::splat(20.0))];

        assert!(grid.aabb_hits(pot, HALF, &obstacles));
        assert!(!grid.aabb_hits(grid.tile_center(2, 1), HALF, &obstacles));

        let start = grid.tile_center(2, 1);
        let moved = grid.move_and_collide_with(&obstacles, HALF, start, Vec2::new(100.0, 0.0));
        assert!(moved.x < pot.x, "should stop before the pot at {moved:?}");
        assert!(!grid.aabb_hits(moved, HALF, &obstacles));
    }

    #[test]
    fn a_pot_can_be_slid_along() {
        let grid = room();
        let pot = grid.tile_center(2, 2);
        let obstacles = [(pot, Vec2::splat(20.0))];
        let start = grid.tile_center(2, 1);

        let moved = grid.move_and_collide_with(&obstacles, HALF, start, Vec2::new(0.0, 100.0));
        assert!(moved.y < pot.y, "should stop above the pot at {moved:?}");
        assert!(!grid.aabb_hits(moved, HALF, &obstacles));
    }

    #[test]
    fn being_inside_a_pot_pushes_out_on_x() {
        let grid = open_room();
        let pot = grid.tile_center(3, 3);
        let obstacles = [(pot, Vec2::splat(20.0))];
        let pushed = grid.push_out_of_obstacles(&obstacles, HALF, Vec2::new(24.0, 0.0));

        assert!(!grid.aabb_hits(pushed, HALF, &obstacles));
        assert_eq!(pushed.y, 0.0, "the free axis should not move");
        assert!(
            pushed.x > pot.x,
            "pushed clear to the right, got {pushed:?}"
        );
    }

    #[test]
    fn being_inside_a_pot_pushes_out_on_y_when_that_is_cheaper() {
        let grid = open_room();
        let pot = grid.tile_center(3, 3);
        let obstacles = [(pot, Vec2::splat(20.0))];
        // overlapping x by 20 and y by 10, so y is the cheaper way out
        let pushed = grid.push_out_of_obstacles(&obstacles, HALF, Vec2::new(20.0, 26.0));

        assert!(!grid.aabb_hits(pushed, HALF, &obstacles));
        assert_eq!(pushed.x, 20.0, "the free axis should not move");
        assert!(pushed.y > pot.y, "pushed clear above, got {pushed:?}");
    }

    #[test]
    fn a_free_player_is_not_pushed_anywhere() {
        let grid = open_room();
        let pot = grid.tile_center(3, 3);
        let obstacles = [(pot, Vec2::splat(20.0))];
        let spot = grid.tile_center(1, 1);
        assert_eq!(grid.push_out_of_obstacles(&obstacles, HALF, spot), spot);
    }

    #[test]
    fn overlap_math_agrees_with_itself() {
        assert!(aabbs_overlap(
            Vec2::ZERO,
            Vec2::splat(16.0),
            Vec2::new(20.0, 0.0),
            Vec2::splat(16.0)
        ));
        assert!(!aabbs_overlap(
            Vec2::ZERO,
            Vec2::splat(16.0),
            Vec2::new(32.0, 0.0),
            Vec2::splat(16.0)
        ));
        assert!(!aabbs_overlap(
            Vec2::ZERO,
            Vec2::splat(16.0),
            Vec2::new(0.0, 40.0),
            Vec2::splat(16.0)
        ));
        assert!(aabbs_overlap(
            Vec2::ZERO,
            Vec2::splat(16.0),
            Vec2::ZERO,
            Vec2::ZERO
        ));
    }

    #[test]
    fn an_offset_room_maps_its_own_origin() {
        let grid = SolidGrid::from_level_at(
            &crate::utils::level_parse::parse("[tiles]\n##\n##\n").unwrap(),
            Vec2::new(100.0, 200.0),
        );
        assert_eq!(grid.tile_min(0, 0), Vec2::new(100.0, 200.0));
        assert_eq!(grid.center(), Vec2::new(132.0, 232.0));
    }

    #[test]
    fn farm_props_land_where_the_file_says() {
        let level = LevelId::Farm.parse().unwrap();
        let grid = SolidGrid::from_level(&level);

        let (col, file_row) = level.requires_single(PropKind::PlayerSpawn).unwrap();
        let spawn = grid.prop_center(&level, col, file_row);
        assert!(!grid.aabb_hits_solid(spawn, Vec2::splat(16.0)));
        assert!(!grid.is_solid_at(spawn));

        for (col, file_row) in level.props_of(PropKind::Pot) {
            let pot = grid.prop_center(&level, col, file_row);
            assert!(
                !grid.aabb_hits_solid(pot, Vec2::splat(20.0)),
                "pot at {col},{file_row} overlaps a wall"
            );
        }
    }

    #[test]
    fn the_farm_border_is_solid_in_world_space_too() {
        let level = LevelId::Farm.parse().unwrap();
        let grid = SolidGrid::from_level(&level);
        assert_eq!(grid.width(), 27);
        assert_eq!(grid.height(), 18);
        assert!(grid.is_solid(0, 0));
        assert!(grid.is_solid(26, 17));
        assert!(grid.is_solid(0, 17));
        assert!(grid.is_solid(26, 0));
    }

    #[test]
    fn the_farm_path_is_walkable_end_to_end() {
        let level = LevelId::Farm.parse().unwrap();
        let grid = SolidGrid::from_level(&level);
        // The farm's interior is wide open, so a middle row walks corner to
        // corner; deriving it keeps the test valid as the layout changes.
        let path_row = level.height / 2;
        let world_row = level.world_row(path_row);

        let start = grid.tile_center(1, world_row);
        let mut position = start;
        for col in 2..level.width - 1 {
            let target = grid.tile_center(col, world_row);
            position = grid.move_and_collide(Vec2::splat(12.0), position, target - position);
            assert!(
                (position - target).length() < 1.0,
                "path blocked heading to column {col}: at {position:?} wanted {target:?}"
            );
        }
    }
}
