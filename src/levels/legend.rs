use std::fmt;

#[derive(bevy::reflect::Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TileKind {
    Void,
    #[default]
    Grass,
    Dirt,
    Water,
    Wall,
}

impl TileKind {
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            ' ' => Some(Self::Void),
            ',' => Some(Self::Grass),
            '.' => Some(Self::Dirt),
            '~' => Some(Self::Water),
            '#' => Some(Self::Wall),
            _ => None,
        }
    }

    pub fn as_char(self) -> char {
        match self {
            Self::Void => ' ',
            Self::Grass => ',',
            Self::Dirt => '.',
            Self::Water => '~',
            Self::Wall => '#',
        }
    }

    pub fn is_solid(self) -> bool {
        matches!(self, Self::Wall | Self::Water)
    }

    pub fn tileset_index(self) -> u16 {
        match self {
            Self::Void => 0xffff,
            Self::Grass => 0,
            Self::Dirt => 1,
            Self::Water => 2,
            Self::Wall => 3,
        }
    }

    pub const ALL: [Self; 5] = [Self::Void, Self::Grass, Self::Dirt, Self::Water, Self::Wall];
}

impl fmt::Display for TileKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = write!(f, "{}", self.as_char());
        Ok(())
    }
}

#[derive(bevy::reflect::Reflect, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PropKind {
    PlayerSpawn,
    Pot,
    CraftingStation,
    ArenaGate,
    BossSpawn,
}

impl PropKind {
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            '^' => Some(Self::PlayerSpawn),
            'p' => Some(Self::Pot),
            's' => Some(Self::CraftingStation),
            'x' => Some(Self::ArenaGate),
            'b' => Some(Self::BossSpawn),
            _ => None,
        }
    }

    pub fn as_char(self) -> char {
        match self {
            Self::PlayerSpawn => '^',
            Self::Pot => 'p',
            Self::CraftingStation => 's',
            Self::ArenaGate => 'x',
            Self::BossSpawn => 'b',
        }
    }
}

impl fmt::Display for PropKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _ = write!(f, "{}", self.as_char());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tile_kind_round_trips_through_a_char() {
        for kind in TileKind::ALL {
            assert_eq!(TileKind::from_char(kind.as_char()), Some(kind));
        }
    }

    #[test]
    fn tile_kinds_reject_prop_markers_and_junk() {
        assert_eq!(TileKind::from_char('p'), None);
        assert_eq!(TileKind::from_char('^'), None);
        assert_eq!(TileKind::from_char('!'), None);
    }

    #[test]
    fn wall_and_water_are_solid() {
        assert!(TileKind::Wall.is_solid());
        assert!(TileKind::Water.is_solid());
        assert!(!TileKind::Grass.is_solid());
        assert!(!TileKind::Dirt.is_solid());
        assert!(!TileKind::Void.is_solid());
    }

    #[test]
    fn void_uses_the_sentinel_index_that_the_chunk_shader_discards() {
        assert_eq!(TileKind::Void.tileset_index(), 0xffff);
    }

    #[test]
    fn every_tile_kind_has_its_own_tileset_layer() {
        let mut layers: Vec<u16> = TileKind::ALL
            .iter()
            .map(|kind| kind.tileset_index())
            .collect();
        layers.sort_unstable();
        let count = layers.len();
        layers.dedup();
        assert_eq!(layers.len(), count);
    }

    #[test]
    fn prop_kinds_round_trip_through_a_char() {
        for kind in [
            PropKind::PlayerSpawn,
            PropKind::Pot,
            PropKind::CraftingStation,
            PropKind::ArenaGate,
            PropKind::BossSpawn,
        ] {
            assert_eq!(PropKind::from_char(kind.as_char()), Some(kind));
        }
    }

    #[test]
    fn prop_kinds_reject_tile_chars() {
        assert_eq!(PropKind::from_char('#'), None);
        assert_eq!(PropKind::from_char(','), None);
        assert_eq!(PropKind::from_char(' '), None);
    }
}
