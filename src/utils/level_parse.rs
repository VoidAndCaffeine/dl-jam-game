use crate::levels::legend::{PropKind, TileKind};
use std::fmt;

pub const DEFAULT_TILE_SIZE: u32 = 32;

#[derive(bevy::reflect::Reflect, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Prop {
    pub kind: PropKind,
    pub x: u32,
    pub y: u32,
}

#[derive(bevy::reflect::Reflect, Clone, Debug, Default, PartialEq)]
pub struct LevelDef {
    pub tile_size: u32,
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<TileKind>,
    pub props: Vec<Prop>,
}

impl LevelDef {
    /// The tile at a **file row** (top-down, row 0 is the first line).
    pub fn tile(&self, col: u32, file_row: u32) -> Option<TileKind> {
        if col >= self.width || file_row >= self.height {
            return None;
        }
        Some(self.tiles[(file_row * self.width + col) as usize])
    }

    /// Solidity at a **file row** (top-down). See [`LevelDef::is_solid_in_world`]
    /// for the bottom-up counterpart.
    pub fn is_solid(&self, col: u32, file_row: u32) -> bool {
        self.tile(col, file_row).is_some_and(TileKind::is_solid)
    }

    /// Solidity at a **world row** (bottom-up, row 0 is the floor).
    pub fn is_solid_in_world(&self, col: u32, world_row: u32) -> bool {
        self.is_solid(col, self.world_row(world_row))
    }

    /// Converts a top-down file row into a bottom-up world row.
    ///
    /// Level files read top-down because that is how people draw, but world
    /// space is y-up (row 0 is the floor). The first row of a file is the top
    /// wall, so it becomes the highest world row.
    pub fn world_row(&self, file_row: u32) -> u32 {
        self.height.saturating_sub(1).saturating_sub(file_row)
    }

    /// Converts a bottom-up world row back into a top-down file row.
    pub fn file_row(&self, world_row: u32) -> u32 {
        self.world_row(world_row)
    }

    pub fn props_of(&self, kind: PropKind) -> Vec<(u32, u32)> {
        self.props
            .iter()
            .filter(|prop| prop.kind == kind)
            .map(|prop| (prop.x, prop.y))
            .collect()
    }

    pub fn prop(&self, kind: PropKind) -> Option<(u32, u32)> {
        self.props
            .iter()
            .find(|prop| prop.kind == kind)
            .map(|prop| (prop.x, prop.y))
    }

    pub fn requires_single(&self, kind: PropKind) -> Result<(u32, u32), LevelParseError> {
        let found = self.props_of(kind);
        match found.len() {
            1 => Ok(found[0]),
            0 => Err(LevelParseError::whole(
                "expected exactly one '{kind}', found none",
            )),
            n => Err(LevelParseError::whole(format!(
                "expected exactly one '{kind}', found {n}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelParseError {
    pub line: usize,
    pub message: String,
}

impl LevelParseError {
    pub fn at(line: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }

    pub fn whole(message: impl Into<String>) -> Self {
        Self {
            line: 0,
            message: message.into(),
        }
    }
}

impl fmt::Display for LevelParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

impl std::error::Error for LevelParseError {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Header,
    Tiles,
    Props,
}

pub fn parse(src: &str) -> Result<LevelDef, LevelParseError> {
    let mut tile_size = DEFAULT_TILE_SIZE;
    let mut section = Section::Header;
    let mut width = 0usize;
    let mut tiles: Vec<TileKind> = Vec::new();
    let mut props: Vec<Prop> = Vec::new();

    for (index, raw) in src.lines().enumerate() {
        let line_no = index + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('#') && section != Section::Tiles {
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            let name = line.trim_matches(['[', ']']);
            section = match name {
                "tiles" => {
                    if !tiles.is_empty() || section == Section::Tiles {
                        return Err(LevelParseError::at(line_no, "duplicate [tiles] section"));
                    }
                    Section::Tiles
                }
                "props" => {
                    if tiles.is_empty() {
                        return Err(LevelParseError::at(
                            line_no,
                            "[props] must come after [tiles]",
                        ));
                    }
                    if section == Section::Props {
                        return Err(LevelParseError::at(line_no, "duplicate [props] section"));
                    }
                    Section::Props
                }
                other => {
                    return Err(LevelParseError::at(
                        line_no,
                        format!("unknown section '{other}', expected [tiles] or [props]"),
                    ));
                }
            };
            continue;
        }

        match section {
            Section::Header => {
                let (key, value) = line.split_once('=').ok_or_else(|| {
                    LevelParseError::at(
                        line_no,
                        format!("expected 'key = value' or a [section], found '{line}'"),
                    )
                })?;
                match key.trim() {
                    "tile_size" => {
                        tile_size = value.trim().parse().map_err(|_| {
                            LevelParseError::at(
                                line_no,
                                format!("tile_size must be a positive integer, found '{value}'"),
                            )
                        })?;
                        if tile_size == 0 {
                            return Err(LevelParseError::at(line_no, "tile_size must not be 0"));
                        }
                    }
                    other => {
                        return Err(LevelParseError::at(
                            line_no,
                            format!("unknown key '{other}', expected tile_size"),
                        ));
                    }
                }
            }
            Section::Tiles => {
                let row = raw.trim_end();
                if width == 0 {
                    width = row.chars().count();
                    if width == 0 {
                        return Err(LevelParseError::at(line_no, "empty tile row"));
                    }
                }
                let pushed = row.chars().count();
                if pushed != width {
                    return Err(LevelParseError::at(
                        line_no,
                        format!("row is {pushed} tiles wide, expected {width}"),
                    ));
                }
                for (column, c) in row.chars().enumerate() {
                    let kind = TileKind::from_char(c).ok_or_else(|| {
                        LevelParseError::at(
                            line_no,
                            format!("unknown tile '{c}' at column {column}"),
                        )
                    })?;
                    tiles.push(kind);
                }
            }
            Section::Props => {
                let prop = parse_prop(line, line_no)?;
                let inside = prop.x < width as u32 && prop.y < (tiles.len() / width) as u32;
                if !inside {
                    return Err(LevelParseError::at(
                        line_no,
                        format!(
                            "prop '{}' at {},{} is outside the {}x{} grid",
                            prop.kind,
                            prop.x,
                            prop.y,
                            width,
                            tiles.len() / width
                        ),
                    ));
                }
                if tiles[(prop.y * width as u32 + prop.x) as usize].is_solid() {
                    return Err(LevelParseError::at(
                        line_no,
                        format!(
                            "prop '{}' at {},{} sits on a solid tile",
                            prop.kind, prop.x, prop.y
                        ),
                    ));
                }
                if props
                    .iter()
                    .any(|other| other.x == prop.x && other.y == prop.y)
                {
                    return Err(LevelParseError::at(
                        line_no,
                        format!("more than one prop at {},{}", prop.x, prop.y),
                    ));
                }
                props.push(prop);
            }
        }
    }

    if tiles.is_empty() {
        return Err(LevelParseError::whole("level has no [tiles] section"));
    }

    let height = tiles.len() / width;
    Ok(LevelDef {
        tile_size,
        width: width as u32,
        height: height as u32,
        tiles,
        props,
    })
}

fn parse_prop(line: &str, line_no: usize) -> Result<Prop, LevelParseError> {
    let malformed =
        |message: String| LevelParseError::at(line_no, format!("{message} in '{line}'"));

    let mut parts = line.split_whitespace();
    let marker = parts
        .next()
        .and_then(|token| token.chars().next())
        .ok_or_else(|| malformed("expected a prop marker".to_string()))?;
    let kind = PropKind::from_char(marker)
        .ok_or_else(|| malformed(format!("unknown prop marker '{marker}'")))?;

    let coords = parts
        .next()
        .ok_or_else(|| malformed(format!("prop '{kind}' is missing its coordinates")))?;
    if parts.next().is_some() {
        return Err(malformed(format!("prop '{kind}' has trailing text")));
    }

    let (x, y) = coords
        .split_once(',')
        .ok_or_else(|| malformed(format!("prop '{kind}' needs 'x,y' coordinates")))?;
    let x = x
        .parse()
        .map_err(|_| malformed(format!("prop '{kind}' has a bad x '{x}'")))?;
    let y = y
        .parse()
        .map_err(|_| malformed(format!("prop '{kind}' has a bad y '{y}'")))?;

    Ok(Prop { kind, x, y })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOM: &str = "\
[tiles]
#####
#,,.#
#.,.#
#~,,#
#####

[props]
^ 3,2
";

    fn error(src: &str) -> LevelParseError {
        parse(src).expect_err("expected the level to be rejected")
    }

    #[test]
    fn parses_a_minimal_room() {
        let level = parse(ROOM).expect("room parses");
        assert_eq!(level.width, 5);
        assert_eq!(level.height, 5);
        assert_eq!(level.tile_size, DEFAULT_TILE_SIZE);
        assert_eq!(level.tile(0, 0), Some(TileKind::Wall));
        assert_eq!(level.tile(1, 1), Some(TileKind::Grass));
        assert_eq!(level.tile(1, 2), Some(TileKind::Dirt));
        assert_eq!(level.tile(1, 3), Some(TileKind::Water));
    }

    #[test]
    fn rows_are_top_down_and_tiles_are_row_major() {
        assert!(parse("[tiles]\nAB\nCD\n").is_err());

        let level = parse(ROOM).unwrap();
        assert_eq!(level.tiles.len(), 25);
        assert_eq!(level.tile(4, 0), Some(TileKind::Wall));
        assert_eq!(level.tile(0, 4), Some(TileKind::Wall));
        assert_eq!(level.tile(4, 4), Some(TileKind::Wall));
    }

    #[test]
    fn spaces_become_void_tiles() {
        let level = parse("[tiles]\n# #\n").unwrap();
        assert_eq!(level.width, 3);
        assert_eq!(level.height, 1);
        assert_eq!(level.tile(1, 0), Some(TileKind::Void));
        assert!(!level.is_solid(1, 0));
    }

    #[test]
    fn tile_size_comes_from_the_header() {
        let level = parse("tile_size = 48\n[tiles]\n##\n").unwrap();
        assert_eq!(level.tile_size, 48);
    }

    #[test]
    fn header_comments_and_blank_lines_are_ignored() {
        let level = parse("# a comment\n\n   \ntile_size=16\n\n[tiles]\n##\n##\n").unwrap();
        assert_eq!(level.tile_size, 16);
        assert_eq!(level.width, 2);
        assert_eq!(level.height, 2);
    }

    #[test]
    fn hash_is_a_wall_inside_the_tile_grid_not_a_comment() {
        let level = parse("[tiles]\n###\n#.#\n###\n").unwrap();
        assert_eq!(level.width, 3);
        assert_eq!(level.height, 3);
        assert_eq!(level.tile(0, 0), Some(TileKind::Wall));
        assert!(level.is_solid(0, 0));
        assert_eq!(level.tile(1, 1), Some(TileKind::Dirt));
        assert!(!level.is_solid(1, 1));
    }

    #[test]
    fn comments_are_allowed_after_the_tile_grid() {
        let level =
            parse("[tiles]\n###\n#.#\n[props]\n# where the player starts\n^ 1,1\n").unwrap();
        assert_eq!(level.prop(PropKind::PlayerSpawn), Some((1, 1)));
    }

    #[test]
    fn unknown_header_key_is_rejected() {
        let err = error("tile_sizee = 32\n[tiles]\n#\n");
        assert!(err.message.contains("unknown key"), "{err}");
    }

    #[test]
    fn a_missing_equals_sign_is_rejected() {
        let err = error("tile_size 32\n[tiles]\n#\n");
        assert!(err.message.contains("key = value"), "{err}");
    }

    #[test]
    fn a_non_numeric_tile_size_is_rejected() {
        let err = error("tile_size = big\n[tiles]\n#\n");
        assert!(err.message.contains("tile_size"), "{err}");
    }

    #[test]
    fn a_zero_tile_size_is_rejected() {
        let err = error("tile_size = 0\n[tiles]\n#\n");
        assert!(err.message.contains("tile_size"), "{err}");
    }

    #[test]
    fn a_missing_tiles_section_is_rejected() {
        let err = error("tile_size = 32\n");
        assert_eq!(err.line, 0);
        assert!(err.message.contains("[tiles]"), "{err}");
    }

    #[test]
    fn an_unknown_section_is_rejected() {
        let err = error("[tiles]\n#\n[decor]\n#\n");
        assert!(err.message.contains("unknown section"), "{err}");
        assert_eq!(err.line, 3);
    }

    #[test]
    fn a_duplicate_tiles_section_is_rejected() {
        let err = error("[tiles]\n#\n[tiles]\n#\n");
        assert!(err.message.contains("duplicate"), "{err}");
    }

    #[test]
    fn props_before_tiles_are_rejected() {
        let err = error("[props]\n^ 0,0\n[tiles]\n#\n");
        assert!(err.message.contains("after [tiles]"), "{err}");
    }

    #[test]
    fn ragged_rows_are_rejected_with_the_offending_line() {
        let err = error("[tiles]\n###\n##\n");
        assert_eq!(err.line, 3);
        assert!(err.message.contains("2 tiles wide"), "{err}");
    }

    #[test]
    fn an_unknown_tile_char_is_rejected_with_its_column() {
        let err = error("[tiles]\n#%#\n");
        assert_eq!(err.line, 2);
        assert!(err.message.contains("unknown tile '%'"), "{err}");
        assert!(err.message.contains("column 1"), "{err}");
    }

    #[test]
    fn props_are_read_in_file_order() {
        let src =
            "[tiles]\n#####\n#,,,#\n#,,,#\n#,,,#\n#####\n[props]\n^ 1,3\np 1,1\np 2,1\np 3,1\n";
        let level = parse(src).unwrap();
        assert_eq!(level.prop(PropKind::PlayerSpawn), Some((1, 3)));
        assert_eq!(level.props_of(PropKind::Pot), vec![(1, 1), (2, 1), (3, 1)]);
    }

    #[test]
    fn the_props_section_is_optional() {
        let level = parse("[tiles]\n#\n").unwrap();
        assert!(level.props.is_empty());
        assert_eq!(level.prop(PropKind::Pot), None);
    }

    #[test]
    fn a_prop_outside_the_grid_is_rejected() {
        let err = error("[tiles]\n###\n#.#\n###\n[props]\np 3,1\n");
        assert!(err.message.contains("outside"), "{err}");
    }

    #[test]
    fn a_prop_on_a_solid_tile_is_rejected() {
        let err = error("[tiles]\n###\n###\n###\n[props]\np 1,1\n");
        assert!(err.message.contains("solid"), "{err}");
    }

    #[test]
    fn a_prop_on_water_is_rejected_because_water_is_solid() {
        let err = error("[tiles]\n###\n#~#\n###\n[props]\n^ 1,1\n");
        assert!(err.message.contains("solid"), "{err}");
    }

    #[test]
    fn two_props_on_one_tile_are_rejected() {
        let err = error("[tiles]\n###\n#.#\n###\n[props]\np 1,1\ns 1,1\n");
        assert!(err.message.contains("more than one prop"), "{err}");
    }

    #[test]
    fn an_unknown_prop_marker_is_rejected() {
        let err = error("[tiles]\n###\n#.#\n###\n[props]\nz 1,1\n");
        assert!(err.message.contains("unknown prop marker"), "{err}");
    }

    #[test]
    fn a_prop_without_coordinates_is_rejected() {
        let err = error("[tiles]\n###\n#.#\n###\n[props]\np\n");
        assert!(err.message.contains("missing its coordinates"), "{err}");
    }

    #[test]
    fn a_prop_with_malformed_coordinates_is_rejected() {
        let err = error("[tiles]\n###\n#.#\n###\n[props]\np 1\n");
        assert!(err.message.contains("x,y"), "{err}");

        let err = error("[tiles]\n###\n#.#\n###\n[props]\np 1;\n");
        assert!(err.message.contains("x,y"), "{err}");

        let err = error("[tiles]\n###\n#.#\n###\n[props]\np x,1\n");
        assert!(err.message.contains("bad x"), "{err}");

        let err = error("[tiles]\n###\n#.#\n###\n[props]\np 1,x\n");
        assert!(err.message.contains("bad y"), "{err}");
    }

    #[test]
    fn a_prop_with_trailing_text_is_rejected() {
        let err = error("[tiles]\n###\n#.#\n###\n[props]\np 1,1 oops\n");
        assert!(err.message.contains("trailing text"), "{err}");
    }

    #[test]
    fn requires_single_counts_the_matches() {
        let grid = "[tiles]\n###\n#.#\n#.#\n###\n[props]\n";
        let level = parse(&format!("{grid}^ 1,1\n^ 1,2\n")).unwrap();
        let err = level.requires_single(PropKind::PlayerSpawn).unwrap_err();
        assert!(err.message.contains("found 2"), "{err}");

        let err = level.requires_single(PropKind::Pot).unwrap_err();
        assert!(err.message.contains("found none"), "{err}");

        let level = parse(&format!("{grid}^ 1,1\n")).unwrap();
        assert_eq!(level.requires_single(PropKind::PlayerSpawn), Ok((1, 1)));
    }

    #[test]
    fn out_of_range_lookups_return_none() {
        let level = parse(ROOM).unwrap();
        assert_eq!(level.tile(99, 0), None);
        assert_eq!(level.tile(0, 99), None);
        assert!(!level.is_solid(99, 99));
    }

    #[test]
    fn world_rows_are_the_file_rows_flipped() {
        let level = parse(ROOM).unwrap();
        assert_eq!(level.height, 5);
        assert_eq!(level.world_row(0), 4);
        assert_eq!(level.world_row(4), 0);
        assert_eq!(level.world_row(2), 2);
        for row in 0..level.height {
            assert_eq!(level.file_row(level.world_row(row)), row);
        }
    }

    #[test]
    fn the_top_row_of_a_file_is_the_top_row_of_the_world() {
        let level = parse("[tiles]\n#..\n...\n").unwrap();
        assert!(level.is_solid(0, 0));
        assert!(!level.is_solid(0, 1));

        assert_eq!(level.world_row(0), 1);
        assert!(level.is_solid_in_world(0, level.world_row(0)));
        assert!(!level.is_solid_in_world(0, 0));
    }

    #[test]
    fn world_rows_saturate_instead_of_panicking() {
        let level = parse("[tiles]\n#\n").unwrap();
        assert_eq!(level.world_row(0), 0);
        assert_eq!(level.world_row(99), 0);
    }

    #[test]
    fn errors_print_their_line_number() {
        assert_eq!(LevelParseError::at(7, "boom").to_string(), "line 7: boom");
        assert_eq!(LevelParseError::whole("boom").to_string(), "boom");
    }
}
