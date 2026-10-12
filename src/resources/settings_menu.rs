use crate::resources::settings::{
    ACCESSIBILITY_ROWS, AUDIO_ROWS, Action, DISPLAY_ROWS, KeySlot, SettingRow,
};
use bevy::prelude::*;

/// One page of the settings overlay.
#[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Audio,
    Accessibility,
    Display,
    Controls,
}

impl SettingsTab {
    pub const ALL: [SettingsTab; 4] = [
        SettingsTab::Audio,
        SettingsTab::Accessibility,
        SettingsTab::Display,
        SettingsTab::Controls,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SettingsTab::Audio => "Audio",
            SettingsTab::Accessibility => "Access",
            SettingsTab::Display => "Display",
            SettingsTab::Controls => "Controls",
        }
    }

    /// Cycles to another tab, wrapping.
    pub fn next(self, delta: i32) -> Self {
        let index = SettingsTab::ALL
            .iter()
            .position(|tab| *tab == self)
            .unwrap_or(0) as i32;
        let count = SettingsTab::ALL.len() as i32;
        SettingsTab::ALL[(index + delta).rem_euclid(count) as usize]
    }

    /// The rows this tab shows, in order. Controls is empty: its entries are
    /// key bindings, not stepper rows.
    pub fn rows(self) -> &'static [SettingRow] {
        match self {
            SettingsTab::Audio => AUDIO_ROWS,
            SettingsTab::Accessibility => ACCESSIBILITY_ROWS,
            SettingsTab::Display => DISPLAY_ROWS,
            SettingsTab::Controls => &[],
        }
    }

    /// How many navigable entries precede Back.
    pub fn entry_count(self) -> usize {
        match self {
            // Two slots per action, plus the reset entry.
            SettingsTab::Controls => Action::REBINDABLE.len() * 2 + 1,
            other => other.rows().len(),
        }
    }
}

/// What a navigable index in the active tab points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsItem {
    /// A stepper row.
    Row(SettingRow),
    /// One of an action's two key slots, waiting to be rebound.
    Binding(Action, KeySlot),
    /// The "reset keybindings" entry.
    Reset,
    Back,
}

/// The settings overlay's state. It is shared by the title screen and the pause
/// overlay, so it carries no `mode`: whichever menu opened it, closing simply
/// returns to the menu underneath.
#[derive(Resource, Reflect, Debug, Clone, Default)]
pub struct SettingsMenu {
    pub open: bool,
    /// The highlighted index within the active tab (its entries, then Back).
    pub selected: usize,
    /// Set for the frame the overlay opens, so the key that opened it cannot
    /// also act on the freshly-spawned rows.
    pub just_opened: bool,
    pub tab: SettingsTab,
    /// The slot awaiting a key press, if a rebind is in progress.
    pub listening: Option<(Action, KeySlot)>,
}

impl SettingsMenu {
    pub fn open_menu(&mut self) {
        self.open = true;
        self.selected = 0;
        self.just_opened = true;
        self.tab = SettingsTab::default();
        self.listening = None;
    }

    pub fn close_menu(&mut self) {
        self.open = false;
        self.listening = None;
    }

    /// The rows the active tab shows.
    pub fn rows(&self) -> &'static [SettingRow] {
        self.tab.rows()
    }

    /// The index of the Back entry: one past the active tab's entries.
    pub fn back_index(&self) -> usize {
        self.tab.entry_count()
    }

    /// The total number of navigable entries (entries plus Back).
    pub fn navigable_len(&self) -> usize {
        self.tab.entry_count() + 1
    }

    /// The flat index of a key slot, matching [`Self::item`].
    pub fn slot_index(action: Action, slot: KeySlot) -> Option<usize> {
        let action = Action::REBINDABLE
            .iter()
            .position(|candidate| *candidate == action)?;
        Some(action * 2 + usize::from(slot == KeySlot::Secondary))
    }

    /// The index of the reset entry.
    pub fn reset_index(&self) -> usize {
        Action::REBINDABLE.len() * 2
    }

    /// What the highlighted index points at.
    pub fn item(&self, index: usize) -> Option<SettingsItem> {
        let count = self.tab.entry_count();
        if index < count {
            return match self.tab {
                SettingsTab::Controls => {
                    let slots = Action::REBINDABLE.len() * 2;
                    if index < slots {
                        let action = Action::REBINDABLE[index / 2];
                        let slot = if index.is_multiple_of(2) {
                            KeySlot::Primary
                        } else {
                            KeySlot::Secondary
                        };
                        Some(SettingsItem::Binding(action, slot))
                    } else {
                        Some(SettingsItem::Reset)
                    }
                }
                other => other.rows().get(index).copied().map(SettingsItem::Row),
            };
        }
        if index == count {
            return Some(SettingsItem::Back);
        }
        None
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
    fn opening_resets_the_tab_selection_and_guards_the_opening_key() {
        let mut menu = SettingsMenu {
            selected: 3,
            tab: SettingsTab::Controls,
            ..SettingsMenu::default()
        };
        menu.open_menu();
        assert!(menu.open);
        assert_eq!(menu.selected, 0);
        assert_eq!(menu.tab, SettingsTab::Audio);
        assert!(menu.just_opened);
        assert!(menu.listening.is_none());

        menu.close_menu();
        assert!(!menu.open);
    }

    #[test]
    fn the_audio_tab_reaches_its_rows_then_back() {
        let menu = SettingsMenu::default();
        assert_eq!(menu.rows().len(), 3);
        assert_eq!(menu.back_index(), 3);
        assert_eq!(menu.navigable_len(), 4);
        assert_eq!(
            menu.item(0),
            Some(SettingsItem::Row(SettingRow::MasterVolume))
        );
        assert_eq!(menu.item(3), Some(SettingsItem::Back));
        assert_eq!(menu.item(4), None);
    }

    #[test]
    fn the_controls_tab_lists_both_slots_per_action_then_reset_and_back() {
        let menu = SettingsMenu {
            tab: SettingsTab::Controls,
            ..SettingsMenu::default()
        };
        assert_eq!(
            menu.item(0),
            Some(SettingsItem::Binding(Action::MoveUp, KeySlot::Primary))
        );
        assert_eq!(
            menu.item(1),
            Some(SettingsItem::Binding(Action::MoveUp, KeySlot::Secondary))
        );
        assert_eq!(
            menu.item(2),
            Some(SettingsItem::Binding(Action::MoveDown, KeySlot::Primary))
        );
        assert_eq!(menu.item(menu.reset_index()), Some(SettingsItem::Reset));
        assert_eq!(menu.item(menu.back_index()), Some(SettingsItem::Back));
        assert_eq!(menu.item(menu.navigable_len()), None, "nothing past Back");
    }

    #[test]
    fn slot_indices_round_trip_through_item() {
        for action in Action::REBINDABLE {
            for slot in KeySlot::ALL {
                let index = SettingsMenu::slot_index(action, slot).unwrap();
                assert_eq!(menu_item(index), SettingsItem::Binding(action, slot));
            }
        }
    }

    fn menu_item(index: usize) -> SettingsItem {
        let menu = SettingsMenu {
            tab: SettingsTab::Controls,
            ..SettingsMenu::default()
        };
        menu.item(index).unwrap()
    }

    #[test]
    fn tabs_cycle_in_both_directions() {
        assert_eq!(SettingsTab::Audio.next(1), SettingsTab::Accessibility);
        assert_eq!(SettingsTab::Controls.next(1), SettingsTab::Audio);
        assert_eq!(SettingsTab::Audio.next(-1), SettingsTab::Controls);
    }
}
