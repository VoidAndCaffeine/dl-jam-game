//! Awards points for the run's progress.
//!
//! Points are earned from harvesting crops and felling bosses. The tally stays
//! locked until the dual boss first falls, so this system simply forwards the
//! game's existing messages into [`RunPoints`], which ignores them while locked.

use crate::components::boss::BossId;
use crate::events::{BossDefeated, CropHarvested};
use crate::resources::points::{BOSS_KILL_POINTS, RunPoints};
use bevy::ecs::message::MessageReader;
use bevy::prelude::*;

pub struct PointsPlugin;

impl Plugin for PointsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RunPoints>()
            .register_type::<RunPoints>()
            .add_message::<CropHarvested>()
            .add_message::<BossDefeated>()
            .add_systems(Update, award_points);
    }
}

/// Turns harvests and boss kills into points. Reads the same messages the goals
/// and drop systems use, so no gameplay system needs to know about points.
///
/// The dual boss is awarded in the defeat handler alongside its unlock, so it is
/// skipped here to keep each kill scored in exactly one place.
fn award_points(
    mut harvested: MessageReader<CropHarvested>,
    mut defeated: MessageReader<BossDefeated>,
    mut points: ResMut<RunPoints>,
) {
    for event in harvested.read() {
        points.add(event.crop.points());
    }
    for event in defeated.read() {
        if event.0 != BossId::Dual {
            points.add(BOSS_KILL_POINTS);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::boss::BossId;
    use crate::components::pot::CropType;
    use bevy::state::app::StatesPlugin;

    fn setup_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin, PointsPlugin));
        app
    }

    fn harvest(app: &mut App, crop: CropType) {
        app.world_mut().write_message(CropHarvested {
            crop,
            position: Vec2::ZERO,
        });
        app.update();
    }

    fn kill(app: &mut App, id: BossId) {
        app.world_mut().write_message(BossDefeated(id));
        app.update();
    }

    fn total(app: &App) -> u32 {
        app.world().resource::<RunPoints>().total
    }

    #[test]
    fn nothing_is_earned_while_locked() {
        let mut app = setup_app();
        harvest(&mut app, CropType::Starter);
        kill(&mut app, BossId::BossA);
        assert_eq!(total(&app), 0);
    }

    #[test]
    fn harvests_and_kills_add_up_once_unlocked() {
        let mut app = setup_app();
        app.world_mut().resource_mut::<RunPoints>().unlock();

        harvest(&mut app, CropType::Starter);
        harvest(&mut app, CropType::CropA);
        harvest(&mut app, CropType::CropB);
        kill(&mut app, BossId::BossA);

        assert_eq!(total(&app), 1 + 2 + 3 + BOSS_KILL_POINTS);
    }
}
