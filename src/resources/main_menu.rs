use bevy::prelude::*;

/// The title screen's selection state. The screen is always shown while
/// `GameState::MainMenu` is active, so there is no `open` flag: only the
/// highlighted button and any transient notice.
#[derive(Resource, Reflect, Default, Debug, Clone)]
pub struct MainMenu {
    pub selected: usize,
    pub notice: String,
}

impl MainMenu {
    pub fn set_notice(&mut self, notice: impl Into<String>) {
        self.notice = notice.into();
    }
}

/// Whether the title screen's reserved buttons are shown. They are hidden for
/// now but keep their layout room, so flipping this reveals them.
#[derive(Resource, Reflect, Debug, Clone, Copy, Default)]
pub struct MenuFeatureFlags {
    pub difficulty: bool,
    pub load_game: bool,
    pub infinite_mode: bool,
}

/// Emitted when the player confirms `New Game` on the title screen. A handler
/// resets the run before play begins.
#[derive(Message, Debug, Clone, Copy)]
pub struct NewGameRequested;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_menu_starts_on_the_first_button() {
        let menu = MainMenu::default();
        assert_eq!(menu.selected, 0);
        assert!(menu.notice.is_empty());
    }

    #[test]
    fn reserved_features_default_to_hidden() {
        let flags = MenuFeatureFlags::default();
        assert!(!flags.difficulty);
        assert!(!flags.load_game);
        assert!(!flags.infinite_mode);
    }
}
