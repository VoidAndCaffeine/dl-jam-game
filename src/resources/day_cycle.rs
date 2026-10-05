use bevy::prelude::*;

/// How a boss attempt ended, shown on the result screen.
#[derive(Reflect, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    #[default]
    Victory,
    Defeat,
}

impl Outcome {
    pub fn title(self) -> &'static str {
        match self {
            Outcome::Victory => "VICTORY",
            Outcome::Defeat => "DEFEAT",
        }
    }
}

/// Tracks the between-days flow. Crop growth and the day counter advance only
/// when `pending_advance` is set, so backing out of boss select does not burn a
/// day.
#[derive(Resource, Reflect, Default, Debug, Clone)]
pub struct DayCycle {
    pub pending_advance: bool,
    pub outcome: Option<Outcome>,
    /// Set when the dual boss falls, so the result screen ends the run.
    pub run_complete: bool,
}

impl DayCycle {
    pub fn finish(&mut self, outcome: Outcome) {
        self.outcome = Some(outcome);
    }

    pub fn request_advance(&mut self) {
        self.pending_advance = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_cycle_advances_nothing() {
        let cycle = DayCycle::default();
        assert!(!cycle.pending_advance);
        assert_eq!(cycle.outcome, None);
        assert!(!cycle.run_complete);
    }

    #[test]
    fn finishing_records_the_outcome() {
        let mut cycle = DayCycle::default();
        cycle.finish(Outcome::Defeat);
        assert_eq!(cycle.outcome, Some(Outcome::Defeat));
    }

    #[test]
    fn requesting_an_advance_raises_the_flag() {
        let mut cycle = DayCycle::default();
        cycle.request_advance();
        assert!(cycle.pending_advance);
    }

    #[test]
    fn outcome_titles_are_distinct() {
        assert_eq!(Outcome::Victory.title(), "VICTORY");
        assert_eq!(Outcome::Defeat.title(), "DEFEAT");
    }
}
