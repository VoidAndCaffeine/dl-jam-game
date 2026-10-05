use crate::components::boss::BossId;
use bevy::prelude::*;

/// Drives the boss selection panel. The panel is a menu, not a separate screen:
/// opening it does not by itself load an arena.
#[derive(Resource, Reflect, Default, Debug, Clone)]
pub struct BossSelectMenu {
    pub open: bool,
    /// Index into [`BossId::ALL`].
    pub selected: usize,
    pub confirmation_open: bool,
    pub pending_boss: Option<BossId>,
}

impl BossSelectMenu {
    pub fn open_menu(&mut self) {
        self.open = true;
        self.selected = 0;
        self.confirmation_open = false;
        self.pending_boss = None;
    }

    pub fn close_menu(&mut self) {
        self.open = false;
        self.selected = 0;
        self.confirmation_open = false;
        self.pending_boss = None;
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
}
