use bevy::prelude::*;

/// Dev-only toggles, flipped at runtime (F3 for the collision boxes).
#[derive(Resource, Reflect, Debug, Default)]
pub struct DebugOverlay {
    pub show_aabbs: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overlay_starts_off() {
        assert!(!DebugOverlay::default().show_aabbs);
    }
}
