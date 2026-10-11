//! Platform save storage.
//!
//! The game's run state is serialized to bytes once (see
//! [`crate::resources::save_manager`]) and then handed to one of these backends,
//! which only deal in opaque blobs keyed by a string. Native writes a file;
//! the browser writes `localStorage`. Both are synchronous, so saving and
//! loading fit Bevy's ordinary systems with no async plumbing.
//!
//! Every backend also has a [`MemoryBackend`] twin for tests, so the round-trip
//! logic can be exercised without touching the filesystem or a browser.

use std::collections::HashMap;
use std::sync::Mutex;

/// Why a save or load failed. Deliberately small: the game only ever logs it.
#[derive(Debug)]
pub enum SaveError {
    /// A backend-level failure (filesystem or browser storage).
    Backend(String),
    /// The bytes could not be turned into (or out of) a save payload.
    Encode(String),
    /// Nothing has been saved under the requested key.
    NotFound,
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::Backend(message) => write!(f, "save backend error: {message}"),
            SaveError::Encode(message) => write!(f, "save encode error: {message}"),
            SaveError::NotFound => write!(f, "no save found"),
        }
    }
}

impl std::error::Error for SaveError {}

/// A place a save blob can live. Implementations must be safe to share across
/// threads (Bevy systems may touch the resource from any).
pub trait SaveBackend: Send + Sync {
    fn save(&self, key: &str, data: &[u8]) -> Result<(), SaveError>;
    fn load(&self, key: &str) -> Result<Vec<u8>, SaveError>;
    fn exists(&self, key: &str) -> bool;
    fn delete(&self, key: &str) -> Result<(), SaveError>;
}

/// An in-memory backend. Used by tests and as a fallback if a platform backend
/// cannot be constructed, so saving never hard-fails the game.
#[derive(Default)]
pub struct MemoryBackend {
    slots: Mutex<HashMap<String, Vec<u8>>>,
}

impl SaveBackend for MemoryBackend {
    fn save(&self, key: &str, data: &[u8]) -> Result<(), SaveError> {
        self.slots
            .lock()
            .expect("save memory backend poisoned")
            .insert(key.to_string(), data.to_vec());
        Ok(())
    }

    fn load(&self, key: &str) -> Result<Vec<u8>, SaveError> {
        self.slots
            .lock()
            .expect("save memory backend poisoned")
            .get(key)
            .cloned()
            .ok_or(SaveError::NotFound)
    }

    fn exists(&self, key: &str) -> bool {
        self.slots
            .lock()
            .expect("save memory backend poisoned")
            .contains_key(key)
    }

    fn delete(&self, key: &str) -> Result<(), SaveError> {
        self.slots
            .lock()
            .expect("save memory backend poisoned")
            .remove(key);
        Ok(())
    }
}

/// Native saves: a single file, written whole each time.
#[cfg(not(target_arch = "wasm32"))]
pub struct FileBackend {
    path: std::path::PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileBackend {
    /// Writes next to the working directory. Native is the dev target; the
    /// shipped build is the browser one, which uses `localStorage`.
    pub fn new() -> Self {
        let dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        Self {
            path: dir.join("save.dat"),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for FileBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl SaveBackend for FileBackend {
    fn save(&self, _key: &str, data: &[u8]) -> Result<(), SaveError> {
        std::fs::write(&self.path, data).map_err(|error| SaveError::Backend(error.to_string()))
    }

    fn load(&self, _key: &str) -> Result<Vec<u8>, SaveError> {
        std::fs::read(&self.path).map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => SaveError::NotFound,
            _ => SaveError::Backend(error.to_string()),
        })
    }

    fn exists(&self, _key: &str) -> bool {
        self.path.exists()
    }

    fn delete(&self, _key: &str) -> Result<(), SaveError> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(SaveError::Backend(error.to_string())),
        }
    }
}

/// Browser saves: `localStorage`, base64-armoured so arbitrary bytes survive as
/// a DOM string. One slot per key.
#[cfg(target_arch = "wasm32")]
pub struct LocalStorageBackend;

#[cfg(target_arch = "wasm32")]
impl LocalStorageBackend {
    pub fn new() -> Self {
        Self
    }

    /// The namespaced storage key, so the game never collides with other pages
    /// on the same origin.
    fn storage_key(key: &str) -> String {
        format!("dl-jam:{key}")
    }

    fn storage() -> Result<web_sys::Storage, SaveError> {
        web_sys::window()
            .ok_or_else(|| SaveError::Backend("no window".into()))?
            .local_storage()
            .map_err(|_| SaveError::Backend("localStorage unavailable".into()))?
            .ok_or_else(|| SaveError::Backend("localStorage blocked".into()))
    }
}

#[cfg(target_arch = "wasm32")]
impl Default for LocalStorageBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_arch = "wasm32")]
impl SaveBackend for LocalStorageBackend {
    fn save(&self, key: &str, data: &[u8]) -> Result<(), SaveError> {
        let encoded = base64_encode(data);
        Self::storage()?
            .set_item(&Self::storage_key(key), &encoded)
            .map_err(|_| SaveError::Backend("localStorage write failed".into()))
    }

    fn load(&self, key: &str) -> Result<Vec<u8>, SaveError> {
        let encoded = Self::storage()?
            .get_item(&Self::storage_key(key))
            .map_err(|_| SaveError::Backend("localStorage read failed".into()))?
            .ok_or(SaveError::NotFound)?;
        base64_decode(&encoded).ok_or_else(|| SaveError::Encode("corrupt save".into()))
    }

    fn exists(&self, key: &str) -> bool {
        match Self::storage() {
            Ok(storage) => storage
                .get_item(&Self::storage_key(key))
                .map(|value| value.is_some())
                .unwrap_or(false),
            Err(_) => false,
        }
    }

    fn delete(&self, key: &str) -> Result<(), SaveError> {
        Self::storage()?
            .remove_item(&Self::storage_key(key))
            .map_err(|_| SaveError::Backend("localStorage delete failed".into()))
    }
}

/// The platform's real backend. Tests get an in-memory one so they never touch
/// the developer's disk or a browser's storage.
pub fn platform_backend() -> Box<dyn SaveBackend> {
    #[cfg(test)]
    {
        Box::new(MemoryBackend::default())
    }
    #[cfg(all(not(test), target_arch = "wasm32"))]
    {
        Box::new(LocalStorageBackend::new())
    }
    #[cfg(all(not(test), not(target_arch = "wasm32")))]
    {
        Box::new(FileBackend::new())
    }
}

const BASE64_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with `=` padding. Small and dependency-free, which is all the
/// browser backend needs.
pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64_ALPHABET[(triple >> 18 & 0x3F) as usize] as char);
        out.push(BASE64_ALPHABET[(triple >> 12 & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(BASE64_ALPHABET[(triple >> 6 & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(BASE64_ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Inverse of [`base64_encode`]. Returns `None` on any malformed input.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    fn value(byte: u8) -> Option<u32> {
        match byte {
            b'A'..=b'Z' => Some((byte - b'A') as u32),
            b'a'..=b'z' => Some((byte - b'a' + 26) as u32),
            b'0'..=b'9' => Some((byte - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let cleaned: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();
    if !cleaned.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(cleaned.len() / 4 * 3);
    for chunk in cleaned.chunks(4) {
        let mut values = [0u32; 4];
        let mut padding = 0;
        for (slot, byte) in chunk.iter().enumerate() {
            if *byte == b'=' {
                padding += 1;
                values[slot] = 0;
            } else {
                values[slot] = value(*byte)?;
            }
        }
        if padding > 2 {
            return None;
        }
        let triple = (values[0] << 18) | (values[1] << 12) | (values[2] << 6) | values[3];
        out.push((triple >> 16 & 0xFF) as u8);
        if padding < 2 {
            out.push((triple >> 8 & 0xFF) as u8);
        }
        if padding < 1 {
            out.push((triple & 0xFF) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_backend_round_trips() {
        let backend = MemoryBackend::default();
        assert!(!backend.exists("run"));
        backend.save("run", &[1, 2, 3]).unwrap();
        assert!(backend.exists("run"));
        assert_eq!(backend.load("run").unwrap(), vec![1, 2, 3]);
        backend.delete("run").unwrap();
        assert!(!backend.exists("run"));
        assert!(matches!(backend.load("run"), Err(SaveError::NotFound)));
    }

    #[test]
    fn memory_backend_keeps_keys_separate() {
        let backend = MemoryBackend::default();
        backend.save("a", &[1]).unwrap();
        backend.save("b", &[2]).unwrap();
        assert_eq!(backend.load("a").unwrap(), vec![1]);
        assert_eq!(backend.load("b").unwrap(), vec![2]);
    }

    #[test]
    fn base64_round_trips_every_length_residue() {
        for length in 0..32usize {
            let bytes: Vec<u8> = (0..length).map(|byte| (byte * 7 + 13) as u8).collect();
            let encoded = base64_encode(&bytes);
            assert_eq!(
                base64_decode(&encoded).as_deref(),
                Some(bytes.as_slice()),
                "length {length}"
            );
        }
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_rejects_malformed_input() {
        assert_eq!(base64_decode("Zm9"), None, "length not a multiple of four");
        assert_eq!(base64_decode("!!!!"), None, "illegal character");
    }

    #[test]
    fn error_display_covers_every_variant() {
        assert!(SaveError::NotFound.to_string().contains("no save"));
        assert!(SaveError::Backend("x".into()).to_string().contains('x'));
        assert!(SaveError::Encode("y".into()).to_string().contains('y'));
    }
}
