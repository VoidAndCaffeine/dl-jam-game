//! A one-shot "New crops unlocked" banner shown when the player returns to the
//! farm after a boss reward.

use crate::resources::farm::CropUnlocks;
use crate::states::DayPhase;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

/// How long the banner holds at full opacity before it starts to fade, and how
/// long that fade lasts.
const HOLD: f32 = 1.4;
const FADE: f32 = 0.8;

const NOTICE_Z: i32 = 600;
const FONT_SIZE: f32 = 40.0;

pub struct CropNoticePlugin;

impl Plugin for CropNoticePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CropNoticeState>()
            .add_systems(OnEnter(DayPhase::Farming), announce_unlocks)
            .add_systems(Update, tick_notice);
    }
}

/// The last-seen unlock set. `None` before the first farm visit, so a fresh run
/// never announces the crops it starts with.
#[derive(Resource, Default)]
struct CropNoticeState {
    seen: Option<CropUnlocks>,
}

/// The banner root. `elapsed` drives the hold-then-fade.
#[derive(Component, Debug, Default)]
pub struct CropNotice {
    elapsed: f32,
}

impl CropNotice {
    /// Full opacity through the hold, then a linear fade to nothing.
    fn alpha(&self) -> f32 {
        if self.elapsed <= HOLD {
            1.0
        } else {
            (1.0 - (self.elapsed - HOLD) / FADE).clamp(0.0, 1.0)
        }
    }
}

/// Shows the banner on the farm if a crop was unlocked since the last visit.
fn announce_unlocks(
    mut commands: Commands,
    unlocks: Res<CropUnlocks>,
    mut state: ResMut<CropNoticeState>,
) {
    let newly = state
        .seen
        .as_ref()
        .is_some_and(|seen| gained_a_crop(seen, &unlocks));
    state.seen = Some(unlocks.clone());
    if newly {
        spawn_notice(&mut commands);
    }
}

/// Whether `current` has a crop `previous` did not.
fn gained_a_crop(previous: &CropUnlocks, current: &CropUnlocks) -> bool {
    (current.crop_a && !previous.crop_a) || (current.crop_b && !previous.crop_b)
}

fn spawn_notice(commands: &mut Commands) {
    commands
        .spawn((
            Name::new("Crop Unlock Notice"),
            CropNotice::default(),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            FocusPolicy::Pass,
            GlobalZIndex(NOTICE_Z),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new("New crops unlocked"),
                TextFont {
                    font_size: FontSize::Px(FONT_SIZE),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

/// Fades the banner's text out and despawns the root once it is gone.
fn tick_notice(
    mut commands: Commands,
    time: Res<Time>,
    mut notices: Query<(Entity, &mut CropNotice, &Children)>,
    mut colors: Query<&mut TextColor>,
) {
    let dt = time.delta_secs();
    for (entity, mut notice, children) in notices.iter_mut() {
        notice.elapsed += dt;
        let alpha = notice.alpha();
        for child in children.iter() {
            if let Ok(mut color) = colors.get_mut(child) {
                color.0 = Color::srgba(1.0, 1.0, 1.0, alpha);
            }
        }
        if notice.elapsed >= HOLD + FADE {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::states::GameState;
    use bevy::state::app::StatesPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.init_resource::<CropUnlocks>()
            .add_plugins((MinimalPlugins, StatesPlugin, CropNoticePlugin))
            .init_state::<GameState>()
            .init_state::<DayPhase>();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Playing);
        app.update();
        app
    }

    fn set_phase(app: &mut App, phase: DayPhase) {
        app.world_mut()
            .resource_mut::<NextState<DayPhase>>()
            .set(phase);
        app.update();
    }

    fn notices(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, With<CropNotice>>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn no_banner_on_the_first_visit_to_the_farm() {
        let mut app = setup_app();
        set_phase(&mut app, DayPhase::BossFight);
        set_phase(&mut app, DayPhase::Farming);

        assert_eq!(notices(&mut app), 0, "a fresh run has nothing to announce");
    }

    #[test]
    fn unlocking_a_crop_shows_the_banner_on_the_next_farm_visit() {
        let mut app = setup_app();
        // Prime the snapshot on the first farm visit.
        set_phase(&mut app, DayPhase::BossFight);

        app.world_mut()
            .resource_mut::<CropUnlocks>()
            .unlock_crop_a();
        set_phase(&mut app, DayPhase::Farming);

        assert_eq!(notices(&mut app), 1);
    }

    #[test]
    fn the_banner_fades_and_despawns() {
        let mut app = setup_app();
        set_phase(&mut app, DayPhase::BossFight);
        app.world_mut()
            .resource_mut::<CropUnlocks>()
            .unlock_crop_b();
        set_phase(&mut app, DayPhase::Farming);
        assert_eq!(notices(&mut app), 1);

        let notice = app
            .world_mut()
            .query_filtered::<Entity, With<CropNotice>>()
            .iter(app.world())
            .next()
            .unwrap();
        app.world_mut()
            .get_mut::<CropNotice>(notice)
            .unwrap()
            .elapsed = HOLD + FADE;
        app.update();

        assert_eq!(notices(&mut app), 0, "the banner is gone after its life");
    }

    #[test]
    fn the_banner_holds_then_fades() {
        let mut notice = CropNotice {
            elapsed: HOLD * 0.5,
        };
        assert_eq!(notice.alpha(), 1.0, "full opacity through the hold");
        notice.elapsed = HOLD + FADE * 0.5;
        assert!(
            (notice.alpha() - 0.5).abs() < 1e-4,
            "halfway through the fade"
        );
        notice.elapsed = HOLD + FADE;
        assert_eq!(notice.alpha(), 0.0);
    }

    #[test]
    fn gained_a_crop_only_counts_new_ones() {
        let mut previous = CropUnlocks::new();
        previous.unlock_crop_a();
        assert!(gained_a_crop(&CropUnlocks::new(), &previous));
        assert!(!gained_a_crop(&previous, &previous));
        assert!(
            !gained_a_crop(&previous, &CropUnlocks::new()),
            "a reset is not a gain"
        );
    }
}
