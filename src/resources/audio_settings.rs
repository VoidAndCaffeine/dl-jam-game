use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Per-bus volume, as linear gains.
///
/// The audio layer multiplies every cue by the matching bus, so a settings menu
/// can drive these later without touching a single cue id. The defaults are
/// neutral (unity gain), which leaves today's mix exactly as authored.
#[derive(Resource, Reflect, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AudioSettings {
    pub master: f32,
    pub music: f32,
    pub sfx: f32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            master: 1.0,
            music: 1.0,
            sfx: 1.0,
        }
    }
}

impl AudioSettings {
    /// The gain applied to every sound effect.
    pub fn sfx_gain(&self) -> f32 {
        (self.master * self.sfx).clamp(0.0, 1.0)
    }

    /// The gain applied to every music track.
    pub fn music_gain(&self) -> f32 {
        (self.master * self.music).clamp(0.0, 1.0)
    }

    pub fn silent(&self) -> bool {
        self.master <= 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_unity_so_the_mix_is_unchanged() {
        let settings = AudioSettings::default();
        assert_eq!(settings.sfx_gain(), 1.0);
        assert_eq!(settings.music_gain(), 1.0);
        assert!(!settings.silent());
    }

    #[test]
    fn the_buses_multiply_together() {
        let settings = AudioSettings {
            master: 0.5,
            music: 1.0,
            sfx: 0.8,
        };
        assert!((settings.sfx_gain() - 0.4).abs() < 1e-6);
        assert!((settings.music_gain() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn gains_never_exceed_unity_or_drop_below_zero() {
        let loud = AudioSettings {
            master: 4.0,
            music: 4.0,
            sfx: 4.0,
        };
        assert_eq!(loud.sfx_gain(), 1.0);
        assert_eq!(loud.music_gain(), 1.0);

        let muted = AudioSettings {
            master: 0.0,
            music: 1.0,
            sfx: 1.0,
        };
        assert_eq!(muted.sfx_gain(), 0.0);
        assert!(muted.silent());
    }
}
