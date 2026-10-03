use bevy::prelude::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .run();
}