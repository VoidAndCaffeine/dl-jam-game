use crate::events::PlaySfx;
use bevy::prelude::*;

/// Audio hook layer.
///
/// No sound files ship yet, so this resolves each cue to its filename and logs
/// it at debug level. Swapping in real playback later only touches this plugin.
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySfx>()
            .add_systems(Update, resolve_sfx_cues);
    }
}

/// Logs every requested cue. Debug builds only, so release stays quiet.
fn resolve_sfx_cues(mut cues: MessageReader<PlaySfx>) {
    for cue in cues.read() {
        log::debug!("sfx: {}", cue.0.file());
    }
}
