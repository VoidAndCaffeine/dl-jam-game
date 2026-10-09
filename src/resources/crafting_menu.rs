use bevy::prelude::*;

#[derive(Resource, Reflect, Default, Debug, Clone)]
pub struct CraftingMenu {
    pub open: bool,
    pub selected: usize,
    pub notice: String,
}

impl CraftingMenu {
    pub fn open_menu(&mut self) {
        self.open = true;
        self.selected = 0;
        self.notice.clear();
    }

    pub fn close_menu(&mut self) {
        self.open = false;
        self.selected = 0;
        self.notice.clear();
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
        };
        assert!(menu.open);
        assert_eq!(menu.selected, 2);
        assert_eq!(menu.notice, "kept");
    }
}
