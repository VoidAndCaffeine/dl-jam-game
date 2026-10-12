//! The modal confirm/acknowledge dialog's UI and input.
//!
//! See [`crate::resources::confirm`] for the state. This plugin owns the overlay
//! and turns a choice into a [`ConfirmResolved`] message; the systems that
//! decided to open the dialog read that message and act on it.

use crate::events::{PlaySfx, Sfx};
use crate::plugins::ui_theme::{
    self as theme, ButtonSpec, MenuArt, MenuButton, MenuButtonSelected,
};
use crate::resources::confirm::{ConfirmDialog, ConfirmResolved};
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

/// The dialog's root, used to spawn and tear it down.
#[derive(Component, Reflect, Debug, Default)]
pub struct ConfirmRoot;

/// A clickable dialog option.
#[derive(Component, Reflect, Debug, Clone, Copy)]
pub struct ConfirmButton {
    pub index: usize,
}

/// The dialog's body text, so a refresh can update it if it ever changes.
#[derive(Component, Reflect, Debug, Default)]
pub struct ConfirmBody;

/// Confirm input lives here so pause can order its `Esc` handling before it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConfirmSet;

pub struct ConfirmPlugin;

impl Plugin for ConfirmPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConfirmDialog>()
            .init_resource::<MenuArt>()
            .add_message::<ConfirmResolved>()
            .add_message::<PlaySfx>()
            .add_systems(
                Update,
                (
                    confirm_input,
                    confirm_click,
                    sync_confirm_ui,
                    refresh_confirm_ui,
                )
                    .chain()
                    .in_set(ConfirmSet)
                    .before(theme::MenuStyleSet),
            );
    }
}

/// Keyboard navigation and activation. `Esc` closes without a choice.
fn confirm_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut dialog: ResMut<ConfirmDialog>,
    mut resolved: MessageWriter<ConfirmResolved>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if !dialog.open {
        return;
    }
    // The key that opened the dialog must not also confirm it.
    if dialog.just_opened {
        dialog.just_opened = false;
        return;
    }

    let last = dialog.options.len().saturating_sub(1);
    if crate::plugins::ui_theme::back_pressed(&keys) && dialog.options.len() > 1 {
        dialog.close();
        sfx.write(PlaySfx(Sfx::MenuClose));
        return;
    }

    let up = keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW);
    let down = keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS);
    if up && dialog.selected > 0 {
        dialog.selected -= 1;
        sfx.write(PlaySfx(Sfx::MenuNavigate));
    } else if down && dialog.selected < last {
        dialog.selected += 1;
        sfx.write(PlaySfx(Sfx::MenuNavigate));
    }

    let confirm = keys.just_pressed(KeyCode::Enter)
        || keys.just_pressed(KeyCode::Space)
        || keys.just_pressed(KeyCode::KeyE);
    if confirm {
        let choice = dialog.selected;
        resolve(&mut dialog, &mut resolved, &mut sfx, choice);
    }
}

/// Clicking an option chooses it.
fn confirm_click(
    buttons: Query<(&Interaction, &ConfirmButton), Changed<Interaction>>,
    mut dialog: ResMut<ConfirmDialog>,
    mut resolved: MessageWriter<ConfirmResolved>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if !dialog.open {
        return;
    }
    for (interaction, button) in buttons.iter() {
        if *interaction == Interaction::Pressed {
            resolve(&mut dialog, &mut resolved, &mut sfx, button.index);
            return;
        }
    }
}

/// Closes the dialog and reports the choice.
fn resolve(
    dialog: &mut ConfirmDialog,
    resolved: &mut MessageWriter<ConfirmResolved>,
    sfx: &mut MessageWriter<PlaySfx>,
    choice: usize,
) {
    let kind = dialog.kind;
    dialog.close();
    sfx.write(PlaySfx(Sfx::MenuConfirm));
    resolved.write(ConfirmResolved { kind, choice });
}

fn sync_confirm_ui(
    mut commands: Commands,
    dialog: Res<ConfirmDialog>,
    mut art: ResMut<MenuArt>,
    server: Option<Res<AssetServer>>,
    roots: Query<Entity, With<ConfirmRoot>>,
) {
    let present = !roots.is_empty();
    if dialog.open && !present {
        spawn_confirm(&mut commands, &dialog, &mut art, server.as_deref());
    } else if !dialog.open && present {
        for entity in roots.iter() {
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_confirm(
    commands: &mut Commands,
    dialog: &ConfirmDialog,
    art: &mut MenuArt,
    server: Option<&AssetServer>,
) {
    commands
        .spawn((
            Name::new("Confirm Root"),
            ConfirmRoot,
            theme::screen_root_node(),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            // Above the pause (500) and difficulty (600) overlays.
            GlobalZIndex(700),
            FocusPolicy::Block,
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Name::new("Confirm Panel"),
                    Node {
                        width: Val::Px(520.0),
                        max_width: Val::Percent(92.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(14.0),
                        padding: UiRect::all(Val::Px(26.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(theme::PANEL_BG),
                    BorderColor::all(theme::PANEL_BORDER),
                ))
                .with_children(|panel| {
                    panel.spawn((
                        Name::new("Title"),
                        theme::label(dialog.title.clone(), 28.0, theme::TEXT_PRIMARY),
                    ));
                    panel.spawn((
                        Name::new("Body"),
                        ConfirmBody,
                        theme::label(dialog.body.clone(), 15.0, theme::TEXT_DIM),
                        TextLayout::justify(Justify::Center),
                        Node {
                            max_width: Val::Percent(100.0),
                            ..default()
                        },
                    ));

                    panel
                        .spawn((
                            Name::new("Options"),
                            Node {
                                width: Val::Percent(100.0),
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(8.0),
                                ..default()
                            },
                        ))
                        .with_children(|options| {
                            for (index, label) in dialog.options.iter().enumerate() {
                                let spec = ButtonSpec::new(index, label.clone()).label_size(20.0);
                                let visuals = art.shared_visuals(server);
                                theme::spawn_menu_button(
                                    options,
                                    spec,
                                    visuals,
                                    ConfirmButton { index },
                                    |_| {},
                                );
                            }
                        });
                });
        });
}

fn refresh_confirm_ui(
    dialog: Res<ConfirmDialog>,
    mut buttons: Query<(&ConfirmButton, &mut MenuButton, &mut MenuButtonSelected)>,
) {
    for (button, mut core, mut selected) in buttons.iter_mut() {
        core.enabled = dialog.open;
        selected.0 = dialog.open && button.index == dialog.selected;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::confirm::ConfirmKind;
    use bevy::ecs::message::MessageReader;
    use bevy::state::app::StatesPlugin;

    #[derive(Resource, Default)]
    struct Captured(Vec<ConfirmResolved>);

    fn capture(mut captured: ResMut<Captured>, mut reader: MessageReader<ConfirmResolved>) {
        for event in reader.read() {
            captured.0.push(*event);
        }
    }

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Captured>()
            .add_plugins((MinimalPlugins, StatesPlugin, ConfirmPlugin))
            .add_systems(Update, capture);
        app
    }

    fn dialog(app: &App) -> &ConfirmDialog {
        app.world().resource::<ConfirmDialog>()
    }

    fn open(app: &mut App, options: usize) {
        let options: Vec<String> = (0..options).map(|i| format!("Option {i}")).collect();
        app.world_mut().resource_mut::<ConfirmDialog>().open(
            ConfirmKind::QuitFightToMenu,
            "Leave?",
            "body",
            options,
        );
        // The opening frame is guarded; run it so the next tap acts.
        app.update();
    }

    fn tap(app: &mut App, key: KeyCode) {
        {
            let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            input.release(key);
            input.press(key);
        }
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear_just_pressed(key);
        app.update();
    }

    #[test]
    fn opening_spawns_the_overlay_and_closing_drops_it() {
        let mut app = setup_app();
        open(&mut app, 2);
        assert_eq!(roots(&mut app), 1);

        app.world_mut().resource_mut::<ConfirmDialog>().close();
        app.update();
        assert_eq!(roots(&mut app), 0);
    }

    #[test]
    fn the_opening_frame_does_not_confirm() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<ConfirmDialog>().open(
            ConfirmKind::QuitFightToMenu,
            "Leave?",
            "body",
            vec!["Stay".into(), "Quit".into()],
        );
        // Press Enter on the very frame the dialog appears.
        {
            let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            input.press(KeyCode::Enter);
        }
        app.update();

        assert!(dialog(&app).open, "the opening Enter is ignored");
    }

    #[test]
    fn enter_chooses_the_highlighted_option() {
        let mut app = setup_app();
        open(&mut app, 2);
        tap(&mut app, KeyCode::ArrowDown);
        assert_eq!(dialog(&app).selected, 1);
        tap(&mut app, KeyCode::Enter);

        assert!(!dialog(&app).open);
        let messages = &app.world().resource::<Captured>().0;
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].choice, 1);
        assert_eq!(messages[0].kind, ConfirmKind::QuitFightToMenu);
    }

    #[test]
    fn escape_cancels_without_a_choice() {
        let mut app = setup_app();
        open(&mut app, 2);
        tap(&mut app, KeyCode::Escape);

        assert!(!dialog(&app).open);
        assert!(app.world().resource::<Captured>().0.is_empty());
    }

    #[test]
    fn escape_is_ignored_for_a_single_option() {
        let mut app = setup_app();
        open(&mut app, 1);
        tap(&mut app, KeyCode::Escape);
        assert!(
            dialog(&app).open,
            "an acknowledge-only dialog cannot be dismissed with Esc"
        );
    }

    fn roots(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<ConfirmRoot>>()
            .iter(app.world())
            .count()
    }
}
