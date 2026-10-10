//! Guards against shipping a sound that Bevy's decoder cannot read.
//!
//! OGG files can carry data the Vorbis decoder rejects (for example an embedded
//! cover image stored in the comment header), which only shows up as a runtime
//! panic the first time the clip plays. This walks every audio file and decodes
//! it once, so a bad asset fails the build instead of the game.
//!
//! The `assets/` tree is gitignored, so the test skips cleanly when it is absent.

use bevy::audio::{AudioSource, Decodable};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn collect_ogg(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_ogg(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "ogg") {
            out.push(path);
        }
    }
}

#[test]
fn every_audio_file_decodes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/audio");
    if !root.is_dir() {
        eprintln!("skipping: {} not present", root.display());
        return;
    }

    let mut files = Vec::new();
    collect_ogg(&root, &mut files);
    files.sort();
    assert!(
        !files.is_empty(),
        "no audio files found under {}",
        root.display()
    );

    // Silence the panic hook's stderr noise while probing.
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut failures = Vec::new();
    for path in &files {
        let bytes = fs::read(path).expect("readable audio file");
        let source = AudioSource {
            bytes: Arc::from(bytes.into_boxed_slice()),
        };
        let decoded = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = source.decoder();
        }));
        if decoded.is_err() {
            failures.push(
                path.strip_prefix(&root)
                    .unwrap_or(path)
                    .display()
                    .to_string(),
            );
        }
    }
    std::panic::set_hook(previous_hook);

    assert!(
        failures.is_empty(),
        "these audio files failed to decode: {failures:?}"
    );
}
