use crate::resources::difficulty::Difficulty;
use bevy::prelude::*;

/// Why the difficulty picker was opened. It changes what confirming does and
/// whether `Permadeath` may be chosen.
#[derive(Reflect, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifficultyMenuMode {
    /// Opened from the title screen's `Difficulty` button: edits the default
    /// used the next time a run starts.
    #[default]
    Default,
    /// Opened by `New Game`: confirming starts a fresh run.
    NewGame,
    /// Opened from the pause overlay while a run is live. `Permadeath` is
    /// locked here, and changes apply from the next boss fight onward.
    MidRun,
}

/// The difficulty picker's state. Like the other menus it is a resource flag:
/// setting `open` spawns the overlay, clearing it tears the overlay down.
#[derive(Resource, Reflect, Default, Debug, Clone)]
pub struct DifficultySelectMenu {
    pub open: bool,
    /// Index into [`Difficulty::ALL`].
    pub selected: usize,
    pub mode: DifficultyMenuMode,
    /// Set the frame the menu opens so the very key/click that opened it does
    /// not also confirm a row.
    pub just_opened: bool,
}

impl DifficultySelectMenu {
    pub fn open_menu(&mut self, mode: DifficultyMenuMode, current: Difficulty) {
        self.open = true;
        self.selected = current.index();
        self.mode = mode;
        self.just_opened = true;
    }

    pub fn close_menu(&mut self) {
        self.open = false;
        self.selected = 0;
        self.mode = DifficultyMenuMode::Default;
        self.just_opened = false;
    }

    /// Whether `Permadeath` is off-limits in the current context.
    pub fn permadeath_locked(&self) -> bool {
        self.mode == DifficultyMenuMode::MidRun
    }

    /// Whether the given row may be picked right now.
    pub fn is_enabled(&self, index: usize) -> bool {
        !(self.permadeath_locked() && Difficulty::ALL.get(index) == Some(&Difficulty::Permadeath))
    }

    /// The difficulty the highlight is on.
    pub fn selected_difficulty(&self) -> Difficulty {
        Difficulty::ALL[self.selected.min(Difficulty::ALL.len() - 1)]
    }

    /// The row indices that can be navigated to, in order.
    pub fn enabled_indices(&self) -> Vec<usize> {
        (0..Difficulty::ALL.len())
            .filter(|index| self.is_enabled(*index))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_starts_closed() {
        let menu = DifficultySelectMenu::default();
        assert!(!menu.open);
        assert_eq!(menu.mode, DifficultyMenuMode::Default);
        assert!(!menu.just_opened);
    }

    #[test]
    fn opening_selects_the_current_difficulty_and_flags_a_fresh_frame() {
        let mut menu = DifficultySelectMenu::default();
        menu.open_menu(DifficultyMenuMode::MidRun, Difficulty::Hard);
        assert!(menu.open);
        assert_eq!(menu.selected, Difficulty::Hard.index());
        assert!(menu.just_opened);
    }

    #[test]
    fn closing_resets_everything() {
        let mut menu = DifficultySelectMenu::default();
        menu.open_menu(DifficultyMenuMode::NewGame, Difficulty::Apocalypse);
        menu.close_menu();
        assert!(!menu.open);
        assert_eq!(menu.selected, 0);
        assert_eq!(menu.mode, DifficultyMenuMode::Default);
        assert!(!menu.just_opened);
    }

    #[test]
    fn permadeath_is_only_locked_mid_run() {
        let mut menu = DifficultySelectMenu::default();
        assert!(!menu.permadeath_locked());

        menu.open_menu(DifficultyMenuMode::MidRun, Difficulty::Normal);
        assert!(menu.permadeath_locked());
        assert!(!menu.is_enabled(Difficulty::Permadeath.index()));
        assert_eq!(menu.enabled_indices().len(), Difficulty::ALL.len() - 1);
    }

    #[test]
    fn selected_difficulty_follows_the_highlight() {
        let mut menu = DifficultySelectMenu::default();
        menu.open_menu(DifficultyMenuMode::Default, Difficulty::Normal);
        menu.selected = Difficulty::Permadeath.index();
        assert_eq!(menu.selected_difficulty(), Difficulty::Permadeath);
        menu.selected = 999;
        assert_eq!(
            menu.selected_difficulty(),
            Difficulty::Permadeath,
            "an out-of-range highlight clamps to the last row"
        );
    }
}
