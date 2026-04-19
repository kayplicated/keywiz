//! Kanata `.kbd` config reader.
//!
//! Currently a stub — the old kanata reader produced the legacy
//! `Grid` type, which no longer exists. Porting to the new
//! Keyboard/Layout split needs a thoughtful pass: a kanata config
//! is both physical layout (defsrc) and character mapping (deflayer
//! tokens). The port produces a synthetic keyboard JSON equivalent
//! plus a matching layout, composed through the engine like any
//! other pair.
//!
//! Until ported, the `--kanata` CLI flag returns an error.

use std::path::Path;

#[derive(Debug)]
pub struct KanataError(pub String);

impl std::fmt::Display for KanataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "kanata integration: {}", self.0)
    }
}

/// Load a kanata `.kbd` config. Returns a keyboard + layout pair
/// that the engine can wrap as if they were loaded from JSON5.
///
/// Not yet implemented — see module docs.
pub fn load(_path: &Path, _layer: Option<&str>) -> Result<(), KanataError> {
    Err(KanataError(
        "kanata integration is temporarily disabled during the engine refactor".into(),
    ))
}
