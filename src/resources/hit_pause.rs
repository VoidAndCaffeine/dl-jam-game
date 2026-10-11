use bevy::prelude::*;

/// A short, deliberate freeze after an impactful hit so the blow lands with
/// weight.
///
/// While this is live the world's movement and boss AI skip their fixed step;
/// rendering, audio, UI and effects keep running, so the freeze reads as a punch
/// rather than a stutter. Tuned to a handful of frames, never more.
#[derive(Resource, Reflect, Debug, Default)]
pub struct HitPause {
    remaining: f32,
}

impl HitPause {
    /// Requests a freeze of `seconds`. A longer freeze already running is never
    /// cut short.
    pub fn trigger(&mut self, seconds: f32) {
        self.remaining = self.remaining.max(seconds.max(0.0));
    }

    pub fn is_frozen(&self) -> bool {
        self.remaining > 0.0
    }

    pub fn remaining(&self) -> f32 {
        self.remaining
    }

    /// Runs the freeze down by `dt`, never below zero.
    pub fn tick(&mut self, dt: f32) {
        self.remaining = (self.remaining - dt).max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_pause_is_not_frozen() {
        let pause = HitPause::default();
        assert!(!pause.is_frozen());
        assert_eq!(pause.remaining(), 0.0);
    }

    #[test]
    fn triggering_freezes_until_the_time_runs_out() {
        let mut pause = HitPause::default();
        pause.trigger(0.05);
        assert!(pause.is_frozen());

        pause.tick(0.02);
        assert!(pause.is_frozen());
        pause.tick(0.1);
        assert!(!pause.is_frozen(), "never goes negative");
        assert_eq!(pause.remaining(), 0.0);
    }

    #[test]
    fn a_longer_freeze_is_never_cut_short_by_a_shorter_one() {
        let mut pause = HitPause::default();
        pause.trigger(0.12);
        pause.trigger(0.05);
        assert!(
            (pause.remaining() - 0.12).abs() < 1e-6,
            "the longer freeze wins"
        );
    }

    #[test]
    fn a_negative_trigger_is_ignored() {
        let mut pause = HitPause::default();
        pause.trigger(-1.0);
        assert!(!pause.is_frozen());
    }
}
