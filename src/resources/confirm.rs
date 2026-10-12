//! A small modal confirm/acknowledge dialog.
//!
//! Two things need one: leaving a Permadeath boss fight for the main menu (a
//! yes/no), and acknowledging that a tainted Permadeath save was invalidated on
//! load (a single button). Both are the same widget with different text, so the
//! state is generic and the *meaning* of a choice is left to the caller via
//! [`ConfirmResolved`].

use bevy::prelude::*;

/// What opened the dialog, so a resolver knows how to read the choice.
#[derive(Reflect, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmKind {
    #[default]
    None,
    /// Leaving a Permadeath boss fight for the main menu. Choice 1 quits.
    QuitFightToMenu,
    /// A tainted Permadeath save was invalidated on load. Choice 0 acknowledges.
    TaintNotice,
    /// The post-game systems (points and Infinite Mode) first unlocked. Choice 0
    /// acknowledges.
    UnlockNotice,
}

/// The modal's state. An overlay: pause is a menu, this sits above it.
#[derive(Resource, Reflect, Default, Debug, Clone)]
pub struct ConfirmDialog {
    pub open: bool,
    pub kind: ConfirmKind,
    pub title: String,
    pub body: String,
    pub options: Vec<String>,
    pub selected: usize,
    /// Swallows the key that opened the dialog so it cannot also confirm it.
    pub just_opened: bool,
}

impl ConfirmDialog {
    /// Opens the dialog on its first option.
    pub fn open(
        &mut self,
        kind: ConfirmKind,
        title: impl Into<String>,
        body: impl Into<String>,
        options: Vec<String>,
    ) {
        self.open = true;
        self.kind = kind;
        self.title = title.into();
        self.body = body.into();
        self.options = options;
        self.selected = 0;
        self.just_opened = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.kind = ConfirmKind::None;
        self.selected = 0;
        self.just_opened = false;
    }
}

/// Emitted when the player picks an option. `choice` is the option's index;
/// closing with `Esc` emits nothing.
#[derive(Message, Debug, Clone, Copy)]
pub struct ConfirmResolved {
    pub kind: ConfirmKind,
    pub choice: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_dialog_is_closed() {
        let dialog = ConfirmDialog::default();
        assert!(!dialog.open);
        assert_eq!(dialog.kind, ConfirmKind::None);
    }

    #[test]
    fn opening_selects_the_first_option_and_guards_the_opening_key() {
        let mut dialog = ConfirmDialog::default();
        dialog.open(
            ConfirmKind::QuitFightToMenu,
            "Leave?",
            "You will step down.",
            vec!["Stay".into(), "Quit".into()],
        );

        assert!(dialog.open);
        assert_eq!(dialog.kind, ConfirmKind::QuitFightToMenu);
        assert_eq!(dialog.selected, 0);
        assert!(dialog.just_opened);
        assert_eq!(dialog.options.len(), 2);
    }

    #[test]
    fn closing_clears_everything() {
        let mut dialog = ConfirmDialog::default();
        dialog.open(ConfirmKind::TaintNotice, "t", "b", vec!["Ok".into()]);
        dialog.selected = 2;
        dialog.close();

        assert!(!dialog.open);
        assert_eq!(dialog.kind, ConfirmKind::None);
        assert_eq!(dialog.selected, 0);
        assert!(!dialog.just_opened);
    }
}
