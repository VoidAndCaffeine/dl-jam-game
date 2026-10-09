//! Bakes the list of button-art PNGs that actually exist into the binary.
//!
//! Per-button overrides are optional: a menu button only loads
//! `images/ui/buttons/<skin>_<state>.png` when that file is present at build
//! time. Scanning here (rather than probing with `AssetServer::load`) means a
//! button that has no custom art simply uses the shared frame instead of
//! logging a "path not found" every frame, and it behaves the same on native
//! and wasm.

use std::fs;
use std::path::Path;

fn main() {
    let dir = Path::new("assets/images/ui/buttons");
    let mut files: Vec<String> = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str()
                && name.ends_with(".png")
            {
                files.push(name.to_string());
            }
        }
    }
    files.sort();

    let mut source = String::from(
        "/// Every `*.png` in `assets/images/ui/buttons` at build time.\n\
         pub const BUTTON_ART_FILES: &[&str] = &[\n",
    );
    for file in &files {
        source.push_str(&format!("    {file:?},\n"));
    }
    source.push_str("];\n");

    let out = std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
    fs::write(Path::new(&out).join("button_art.rs"), source).expect("write button_art.rs");

    println!("cargo:rerun-if-changed=assets/images/ui/buttons");
}
