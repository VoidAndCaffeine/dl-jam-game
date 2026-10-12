use bevy::prelude::*;

/// The settings overlay's state. It is shared by the title screen and the pause
/// overlay, so it carries no `mode`: whichever menu opened it, closing simply
/// returns to the menu underneath.
#[derive(Resource, Reflect, Debug, Clone, Default)]
pub struct SettingsMenu {
    pub open: bool,
    /// The highlighted row (an index into [`crate::resources::settings::SETTING_ROWS`],
    /// or [`SettingsMenu::BACK`] for the Back button).
    pub selected: usize,
    /// Set for the frame the overlay opens, so the key that opened it cannot
    /// also act on the freshly-spawned rows.
    pub just_opened: bool,
}

impl SettingsMenu {
    /// The index of the Back button, one past the steppable rows.
    pub const BACK: usize = crate::resources::settings::SETTING_ROWS.len();

    pub fn open_menu(&mut self) {
        self.open = true;
        self.selected = 0;
        self.just_opened = true;
    }

    pub fn close_menu(&mut self) {
        self.open = false;
    }
}

/// Emitted by the title screen's `Settings` button. The settings plugin turns it
/// into an open overlay, so the title screen never has to know about the panel.
#[derive(Message, Debug, Clone, Copy)]
pub struct SettingsRequested;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_resets_the_selection_and_guards_the_opening_key() {
        let mut menu = SettingsMenu {
            selected: 3,
            ..SettingsMenu::default()
        };
        menu.open_menu();
        assert!(menu.open);
        assert_eq!(menu.selected, 0);
        assert!(menu.just_opened);

        menu.close_menu();
        assert!(!menu.open);
    }

    #[test]
    fn back_sits_after_the_last_row() {
        assert_eq!(
            SettingsMenu::BACK,
            crate::resources::settings::SETTING_ROWS.len()
        );
    }
}
