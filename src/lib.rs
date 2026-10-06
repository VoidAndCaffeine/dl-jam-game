use bevy::prelude::*;
use wasm_bindgen::prelude::*;

pub mod components;
pub mod constants;
pub mod default;
pub mod events;
pub mod levels;
pub mod plugins;
pub mod resources;
pub mod states;
pub mod systems;
pub mod utils;

pub use default::GamePlugin;

#[wasm_bindgen(start)]
pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(GamePlugin)
        .run();
}
