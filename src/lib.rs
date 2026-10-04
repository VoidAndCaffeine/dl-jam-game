use bevy::prelude::*;
use wasm_bindgen::prelude::*;

mod components;
mod default;
mod events;
mod plugins;
mod resources;
mod states;
mod systems;

pub use default::GamePlugin;

#[wasm_bindgen(start)]
pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GamePlugin)
        .run();
}
