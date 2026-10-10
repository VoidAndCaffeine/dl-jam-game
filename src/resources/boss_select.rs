use crate::components::boss::BossId;
use bevy::prelude::*;

/// Drives the boss selection panel. The panel is a menu, not a separate screen:
/// opening it does not by itself load an arena.
#[derive(Resource, Reflect, Default, Debug, Clone)]
pub struct BossSelectMenu {
    pub open: bool,
    /// Index into [`BossId::ALL`].
    pub selected: usize,
    /// The option row under the mouse, if any. The concept-art pane follows this
    /// while `selected` keeps the keyboard highlight, so hovering previews a boss
    /// without opening its confirmation. `None` means "show `selected`".
    pub hovered: Option<usize>,
    pub confirmation_open: bool,
    pub pending_boss: Option<BossId>,
}

impl BossSelectMenu {
    pub fn open_menu(&mut self) {
        self.open = true;
        self.selected = 0;
        self.hovered = None;
        self.confirmation_open = false;
        self.pending_boss = None;
    }

    pub fn close_menu(&mut self) {
        self.open = false;
        self.selected = 0;
        self.hovered = None;
        self.confirmation_open = false;
        self.pending_boss = None;
    }

    /// The option the concept pane should show: the hovered one if the mouse is
    /// over a row, otherwise the keyboard selection.
    pub fn preview_index(&self) -> usize {
        self.hovered.unwrap_or(self.selected)
    }

    pub fn open_confirmation(&mut self, boss: BossId) {
        self.confirmation_open = true;
        self.pending_boss = Some(boss);
    }

    pub fn close_confirmation(&mut self) {
        self.confirmation_open = false;
        self.pending_boss = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_starts_closed() {
        let menu = BossSelectMenu::default();
        assert!(!menu.open);
        assert!(!menu.confirmation_open);
        assert_eq!(menu.pending_boss, None);
    }

    #[test]
    fn closing_clears_confirmation_state() {
        let mut menu = BossSelectMenu::default();
        menu.open_menu();
        menu.open_confirmation(BossId::BossB);
        menu.close_menu();

        assert!(!menu.open);
        assert!(!menu.confirmation_open);
        assert_eq!(menu.pending_boss, None);
        assert_eq!(menu.selected, 0);
    }

    #[test]
    fn confirmation_tracks_the_pending_boss() {
        let mut menu = BossSelectMenu::default();
        menu.open_confirmation(BossId::Dual);
        assert!(menu.confirmation_open);
        assert_eq!(menu.pending_boss, Some(BossId::Dual));

        menu.close_confirmation();
        assert!(!menu.confirmation_open);
        assert_eq!(menu.pending_boss, None);
    }

    #[test]
    fn hovered_clears_on_open_and_close_and_feeds_the_preview() {
        let mut menu = BossSelectMenu::default();
        assert_eq!(menu.hovered, None);

        menu.open_menu();
        menu.hovered = Some(2);
        menu.close_menu();
        assert_eq!(menu.hovered, None);

        menu.selected = 1;
        assert_eq!(menu.preview_index(), 1, "no hover falls back to selection");
        menu.hovered = Some(2);
        assert_eq!(menu.preview_index(), 2, "the hovered option wins");
    }
}
