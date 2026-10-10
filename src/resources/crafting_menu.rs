use bevy::prelude::*;

#[derive(Resource, Reflect, Default, Debug, Clone)]
pub struct CraftingMenu {
    pub open: bool,
    pub selected: usize,
    /// The recipe row under the mouse, if any. The preview panes (gear showpiece,
    /// description, materials) follow this while the keyboard `selected` row
    /// keeps the highlight, so hovering previews without crafting. `None` means
    /// "preview whatever `selected` points at".
    pub hovered: Option<usize>,
    pub notice: String,
}

impl CraftingMenu {
    pub fn open_menu(&mut self) {
        self.open = true;
        self.selected = 0;
        self.hovered = None;
        self.notice.clear();
    }

    pub fn close_menu(&mut self) {
        self.open = false;
        self.selected = 0;
        self.hovered = None;
        self.notice.clear();
    }

    /// The row the preview panes should show: the hovered one if the mouse is
    /// over a row, otherwise the keyboard selection.
    pub fn preview_row(&self) -> usize {
        self.hovered.unwrap_or(self.selected)
    }

    pub fn set_notice(&mut self, notice: impl Into<String>) {
        self.notice = notice.into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_starts_closed() {
        let menu = CraftingMenu::default();
        assert!(!menu.open);
        assert_eq!(menu.selected, 0);
        assert!(menu.notice.is_empty());
    }

    #[test]
    fn open_resets_selection_and_notice() {
        let mut menu = CraftingMenu {
            selected: 3,
            ..default()
        };
        menu.set_notice("Crafted Excavator Set");
        menu.close_menu();
        menu.open_menu();

        assert!(menu.open);
        assert_eq!(menu.selected, 0);
        assert!(menu.notice.is_empty());
    }

    #[test]
    fn close_clears_state() {
        let mut menu = CraftingMenu::default();
        menu.open_menu();
        menu.selected = 5;
        menu.set_notice("Missing: Tailings Potato");
        menu.close_menu();

        assert!(!menu.open);
        assert_eq!(menu.selected, 0);
        assert!(menu.notice.is_empty());
    }

    #[test]
    fn set_notice_accepts_strings() {
        let mut menu = CraftingMenu::default();
        menu.set_notice("hello");
        assert_eq!(menu.notice, "hello");
        menu.set_notice(String::from("world"));
        assert_eq!(menu.notice, "world");
    }

    #[test]
    fn menu_defaults_are_consistent() {
        let menu = CraftingMenu {
            open: true,
            selected: 2,
            notice: "kept".to_string(),
            ..default()
        };
        assert!(menu.open);
        assert_eq!(menu.selected, 2);
        assert_eq!(menu.notice, "kept");
    }

    #[test]
    fn hovered_defaults_to_none_and_clears_on_open_and_close() {
        let mut menu = CraftingMenu::default();
        assert_eq!(menu.hovered, None);

        menu.open_menu();
        menu.hovered = Some(4);
        menu.close_menu();
        assert_eq!(menu.hovered, None, "closing drops the hovered row");

        menu.hovered = Some(2);
        menu.open_menu();
        assert_eq!(menu.hovered, None, "opening starts with no row hovered");
    }

    #[test]
    fn preview_row_prefers_the_hovered_row() {
        let mut menu = CraftingMenu {
            selected: 1,
            ..default()
        };
        assert_eq!(menu.preview_row(), 1, "no hover falls back to selection");

        menu.hovered = Some(5);
        assert_eq!(menu.preview_row(), 5, "the hovered row wins");

        menu.hovered = None;
        assert_eq!(menu.preview_row(), 1);
    }
}
