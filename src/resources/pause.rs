use bevy::prelude::*;

/// Drives the pause overlay. Pause is a menu, not a state: it sits on top of
/// whatever `Playing` phase is live and freezes the world by pausing virtual
/// time, so bosses, timers and animations all stop together.
#[derive(Resource, Reflect, Default, Debug, Clone)]
pub struct PauseMenu {
    pub open: bool,
    /// Index into the pause buttons. `Resume` (0) is auto-selected.
    pub selected: usize,
}

impl PauseMenu {
    pub fn open_menu(&mut self) {
        self.open = true;
        self.selected = 0;
    }

    pub fn close_menu(&mut self) {
        self.open = false;
        self.selected = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_starts_closed_on_resume() {
        let menu = PauseMenu::default();
        assert!(!menu.open);
        assert_eq!(menu.selected, 0);
    }

    #[test]
    fn opening_selects_resume_and_closing_resets() {
        let mut menu = PauseMenu::default();
        menu.open_menu();
        assert!(menu.open);
        assert_eq!(menu.selected, 0);

        menu.selected = 2;
        menu.close_menu();
        assert!(!menu.open);
        assert_eq!(menu.selected, 0);
    }
}
