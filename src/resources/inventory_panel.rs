use bevy::prelude::*;

#[derive(Resource, Reflect, Default, Debug, Clone)]
pub struct InventoryPanel {
    pub open: bool,
    pub selected: usize,
    pub notice: String,
}

impl InventoryPanel {
    pub fn open_panel(&mut self) {
        self.open = true;
        self.selected = 0;
        self.notice.clear();
    }

    pub fn close_panel(&mut self) {
        self.open = false;
        self.selected = 0;
        self.notice.clear();
    }

    pub fn toggle_panel(&mut self) -> bool {
        if self.open {
            self.close_panel();
        } else {
            self.open_panel();
        }
        self.open
    }

    pub fn set_notice(&mut self, notice: impl Into<String>) {
        self.notice = notice.into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_starts_closed() {
        let panel = InventoryPanel::default();
        assert!(!panel.open);
        assert_eq!(panel.selected, 0);
        assert!(panel.notice.is_empty());
    }

    #[test]
    fn open_resets_selection_and_notice() {
        let mut panel = InventoryPanel::default();
        panel.selected = 3;
        panel.set_notice("Equipped Wooden Sword");
        panel.close_panel();
        panel.open_panel();

        assert!(panel.open);
        assert_eq!(panel.selected, 0);
        assert!(panel.notice.is_empty());
    }

    #[test]
    fn close_clears_state() {
        let mut panel = InventoryPanel::default();
        panel.open_panel();
        panel.selected = 5;
        panel.set_notice("Ember Saber is already equipped");
        panel.close_panel();

        assert!(!panel.open);
        assert_eq!(panel.selected, 0);
        assert!(panel.notice.is_empty());
    }

    #[test]
    fn toggle_alternates_between_open_and_closed() {
        let mut panel = InventoryPanel::default();

        assert!(panel.toggle_panel());
        assert!(panel.open);
        assert!(!panel.toggle_panel());
        assert!(!panel.open);
    }

    #[test]
    fn toggle_to_open_clears_the_previous_selection() {
        let mut panel = InventoryPanel::default();
        panel.open_panel();
        panel.selected = 4;
        panel.set_notice("notice");
        panel.close_panel();

        assert!(panel.toggle_panel());
        assert_eq!(panel.selected, 0);
        assert!(panel.notice.is_empty());
    }

    #[test]
    fn toggle_to_close_clears_the_previous_selection() {
        let mut panel = InventoryPanel::default();
        panel.open_panel();
        panel.selected = 4;
        panel.set_notice("notice");

        assert!(!panel.toggle_panel());
        assert_eq!(panel.selected, 0);
        assert!(panel.notice.is_empty());
    }

    #[test]
    fn set_notice_accepts_strings() {
        let mut panel = InventoryPanel::default();
        panel.set_notice("hello");
        assert_eq!(panel.notice, "hello");
        panel.set_notice(String::from("world"));
        assert_eq!(panel.notice, "world");
    }

    #[test]
    fn panel_defaults_are_consistent() {
        let panel = InventoryPanel {
            open: true,
            selected: 2,
            notice: "kept".to_string(),
        };
        assert!(panel.open);
        assert_eq!(panel.selected, 2);
        assert_eq!(panel.notice, "kept");
    }
}
