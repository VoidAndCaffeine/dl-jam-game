use bevy::prelude::*;

/// Whether two circles overlap, inclusive of their radii.
pub fn circles_overlap(a: Vec2, a_radius: f32, b: Vec2, b_radius: f32) -> bool {
    (a - b).length_squared() <= (a_radius + b_radius) * (a_radius + b_radius)
}

/// Whether `point` sits inside a cone centred on `origin`, opening along `dir`
/// with half-angle `cone_deg / 2`, within `range`.
pub fn point_in_cone(origin: Vec2, dir: Vec2, cone_deg: f32, range: f32, point: Vec2) -> bool {
    let offset = point - origin;
    if offset.length_squared() > range * range {
        return false;
    }
    let dir = dir.normalize_or(Vec2::X);
    let offset = offset.normalize_or(Vec2::X);
    let half = (cone_deg * 0.5).to_radians();
    dir.dot(offset) >= half.cos()
}

/// The candidate nearest to `origin`, or `None` when there are none.
pub fn nearest_target(origin: Vec2, candidates: &[(Entity, Vec2)]) -> Option<Entity> {
    candidates
        .iter()
        .min_by(|(_, a), (_, b)| {
            a.distance_squared(origin)
                .total_cmp(&b.distance_squared(origin))
        })
        .map(|(entity, _)| *entity)
}

/// Candidates ordered nearest-first.
pub fn ordered_by_distance(origin: Vec2, candidates: &[(Entity, Vec2)]) -> Vec<Entity> {
    let mut sorted = candidates.to_vec();
    sorted.sort_by(|(_, a), (_, b)| {
        a.distance_squared(origin)
            .total_cmp(&b.distance_squared(origin))
    });
    sorted.into_iter().map(|(entity, _)| entity).collect()
}

/// Steps the lock-on selection to the next target by distance.
///
/// With no current target (or one that has vanished), forward picks the nearest
/// and backward picks the farthest. Otherwise it wraps around the distance-sorted
/// list. Returns `None` only when there are no candidates at all.
pub fn cycle_target(
    origin: Vec2,
    candidates: &[(Entity, Vec2)],
    current: Option<Entity>,
    forward: bool,
) -> Option<Entity> {
    let ordered = ordered_by_distance(origin, candidates);
    if ordered.is_empty() {
        return None;
    }
    let Some(index) = current.and_then(|c| ordered.iter().position(|e| *e == c)) else {
        return if forward {
            ordered.first().copied()
        } else {
            ordered.last().copied()
        };
    };
    let step = if forward { 1 } else { ordered.len() - 1 };
    ordered.get((index + step) % ordered.len()).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(world: &mut World) -> Entity {
        world.spawn_empty().id()
    }

    #[test]
    fn circles_touch_at_the_sum_of_radii() {
        assert!(circles_overlap(
            Vec2::ZERO,
            10.0,
            Vec2::new(20.0, 0.0),
            10.0
        ));
        assert!(!circles_overlap(
            Vec2::ZERO,
            10.0,
            Vec2::new(21.0, 0.0),
            10.0
        ));
    }

    #[test]
    fn a_cone_catches_points_ahead_and_rejects_those_behind() {
        assert!(point_in_cone(
            Vec2::ZERO,
            Vec2::X,
            60.0,
            100.0,
            Vec2::new(50.0, 0.0)
        ));
        assert!(point_in_cone(
            Vec2::ZERO,
            Vec2::X,
            60.0,
            100.0,
            Vec2::new(50.0, 20.0)
        ));
        assert!(!point_in_cone(
            Vec2::ZERO,
            Vec2::X,
            60.0,
            100.0,
            Vec2::new(-50.0, 0.0)
        ));
        assert!(!point_in_cone(
            Vec2::ZERO,
            Vec2::X,
            60.0,
            100.0,
            Vec2::new(200.0, 0.0)
        ));
    }

    #[test]
    fn nearest_picks_the_closest_candidate() {
        let mut world = World::new();
        let near = entity(&mut world);
        let far = entity(&mut world);
        let candidates = [(far, Vec2::new(200.0, 0.0)), (near, Vec2::new(20.0, 0.0))];
        assert_eq!(nearest_target(Vec2::ZERO, &candidates), Some(near));
    }

    #[test]
    fn cycling_wraps_around_the_distance_order() {
        let mut world = World::new();
        let a = entity(&mut world);
        let b = entity(&mut world);
        let c = entity(&mut world);
        let candidates = [
            (b, Vec2::new(20.0, 0.0)),
            (a, Vec2::new(10.0, 0.0)),
            (c, Vec2::new(30.0, 0.0)),
        ];

        assert_eq!(cycle_target(Vec2::ZERO, &candidates, None, true), Some(a));
        assert_eq!(
            cycle_target(Vec2::ZERO, &candidates, Some(a), true),
            Some(b)
        );
        assert_eq!(
            cycle_target(Vec2::ZERO, &candidates, Some(b), true),
            Some(c)
        );
        assert_eq!(
            cycle_target(Vec2::ZERO, &candidates, Some(c), true),
            Some(a)
        );
        assert_eq!(
            cycle_target(Vec2::ZERO, &candidates, Some(a), false),
            Some(c)
        );
    }

    #[test]
    fn cycling_with_no_candidates_is_none() {
        assert_eq!(cycle_target(Vec2::ZERO, &[], None, true), None);
    }

    #[test]
    fn cycling_backward_from_no_target_takes_the_farthest() {
        let mut world = World::new();
        let a = entity(&mut world);
        let b = entity(&mut world);
        let candidates = [(a, Vec2::new(10.0, 0.0)), (b, Vec2::new(90.0, 0.0))];
        assert_eq!(cycle_target(Vec2::ZERO, &candidates, None, false), Some(b));
    }
}
