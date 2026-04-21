//! Kanata `.kbd` config reader — **staged, not yet implemented.**
//!
//! Porting from the old Grid-producing reader to the new
//! Keyboard/Layout split: a kanata config is both physical layout
//! (defsrc) and character mapping (deflayer tokens). The port
//! produces a synthetic keyboard + matching layout, composed
//! through the engine like any other pair.
//!
//! Until implemented, the `--kanata` CLI flag errors out in
//! `main.rs`. The types below are the intended shape; callers
//! don't exist yet.

#![allow(dead_code)]

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
        "kanata integration is not yet implemented".into(),
    ))
}
