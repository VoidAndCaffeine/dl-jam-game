use crate::components::pot::CropType;
use bevy::prelude::*;

/// Drives the crop selection panel shown when the player interacts with an empty
/// pot. Planting does not happen until the player confirms a crop, so the target
/// pot is remembered as a [`crate::components::pot::Pot::index`].
#[derive(Resource, Reflect, Default, Debug, Clone)]
pub struct CropSelectMenu {
    pub open: bool,
    /// Index into [`crate::components::pot::CropType::ALL`].
    pub selected: usize,
    /// The crop row under the mouse, if any. The sapling preview follows this
    /// while `selected` keeps the keyboard highlight, so hovering previews a crop
    /// without planting it. `None` means "preview `selected`".
    pub hovered: Option<usize>,
    /// `Pot::index` the picker was opened for.
    pub pending_pot: usize,
    /// The crop the picker should highlight next time it opens: the last crop
    /// planted, or a crop that was just unlocked (until the player plants again).
    pub last_planted: Option<CropType>,
}

impl CropSelectMenu {
    pub fn open_menu(&mut self, pending_pot: usize) {
        self.open = true;
        self.selected = self.default_index();
        self.hovered = None;
        self.pending_pot = pending_pot;
    }

    /// The row to highlight on open: the last-planted (or freshly unlocked) crop
    /// when there is one, otherwise the first crop.
    fn default_index(&self) -> usize {
        self.last_planted
            .and_then(|crop| CropType::ALL.iter().position(|c| *c == crop))
            .unwrap_or(0)
    }

    pub fn close_menu(&mut self) {
        self.open = false;
        self.selected = 0;
        self.hovered = None;
        self.pending_pot = 0;
    }

    /// Clears every field, including the remembered default. Used by `New Game`.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// The crop the preview should show: the hovered one if the mouse is over a
    /// row, otherwise the keyboard selection.
    pub fn preview_index(&self) -> usize {
        self.hovered.unwrap_or(self.selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_starts_closed() {
        let menu = CropSelectMenu::default();
        assert!(!menu.open);
        assert_eq!(menu.selected, 0);
        assert_eq!(menu.pending_pot, 0);
    }

    #[test]
    fn open_tracks_the_target_pot_and_resets_selection() {
        let mut menu = CropSelectMenu {
            selected: 2,
            ..default()
        };
        menu.open_menu(4);

        assert!(menu.open);
        assert_eq!(menu.selected, 0);
        assert_eq!(menu.pending_pot, 4);
    }

    #[test]
    fn open_defaults_to_the_last_planted_crop() {
        let mut menu = CropSelectMenu {
            last_planted: Some(CropType::CropB),
            ..default()
        };
        menu.open_menu(0);
        assert_eq!(
            menu.selected, 2,
            "the picker opens on the last crop planted"
        );
    }

    #[test]
    fn reset_clears_the_remembered_default() {
        let mut menu = CropSelectMenu {
            last_planted: Some(CropType::CropA),
            ..default()
        };
        menu.reset();
        assert_eq!(menu.last_planted, None);
        assert!(!menu.open);
    }

    #[test]
    fn close_clears_state() {
        let mut menu = CropSelectMenu::default();
        menu.open_menu(3);
        menu.selected = 2;
        menu.close_menu();

        assert!(!menu.open);
        assert_eq!(menu.selected, 0);
        assert_eq!(menu.pending_pot, 0);
    }

    #[test]
    fn reopening_targets_the_new_pot() {
        let mut menu = CropSelectMenu::default();
        menu.open_menu(1);
        menu.close_menu();
        menu.open_menu(7);

        assert!(menu.open);
        assert_eq!(menu.pending_pot, 7);
        assert_eq!(menu.selected, 0);
    }

    #[test]
    fn hovered_clears_on_open_and_close_and_feeds_the_preview() {
        let mut menu = CropSelectMenu::default();
        assert_eq!(menu.hovered, None);

        menu.open_menu(0);
        menu.hovered = Some(2);
        menu.close_menu();
        assert_eq!(menu.hovered, None);

        menu.selected = 1;
        assert_eq!(menu.preview_index(), 1, "no hover falls back to selection");
        menu.hovered = Some(2);
        assert_eq!(menu.preview_index(), 2, "the hovered crop wins");
    }
}
