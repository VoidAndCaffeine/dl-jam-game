use bevy::asset::{AssetMetaCheck, AssetPlugin};
use bevy::prelude::*;
use dl_jam::GamePlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            // Servers (trunk's dev server, itch.io) answer missing files with a
            // 200 HTML page instead of a 404, so probing for `<asset>.meta`
            // would feed HTML to the RON parser and fail every load. Skip meta
            // lookups entirely and use each loader's default settings.
            meta_check: AssetMetaCheck::Never,
            ..default()
        }))
        .add_plugins(GamePlugin)
        .run();
}
