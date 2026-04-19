//! Tiny persisted user preferences — remembers the last keyboard +
//! layout pair between sessions so you don't have to pass `-k`/`-l`
//! every launch.
//!
//! Stored as JSON under the OS data directory (same spot as per-layout
//! stats). Missing or unreadable → silent fallback to built-in defaults;
//! a preferences file must never block the app from starting.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Prefs {
    pub keyboard: Option<String>,
    pub layout: Option<String>,
    #[serde(default)]
    pub exercise: Option<String>,
}

impl Prefs {
    /// Load previously-saved preferences. Returns default (empty) prefs
    /// on first run, unreadable file, or parse failure — never errors.
    pub fn load() -> Self {
        let Some(path) = prefs_path() else {
            return Self::default();
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&content).unwrap_or_default()
    }

    /// Save the active keyboard / layout / exercise so the next
    /// launch resumes where the user left off. Failures are logged
    /// to stderr and swallowed — preferences are QoL, never
    /// load-bearing.
    pub fn save(keyboard: &str, layout: &str, exercise: &str) {
        let Some(path) = prefs_path() else {
            return;
        };
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            eprintln!("keywiz: could not create {parent:?}: {e}");
            return;
        }
        let prefs = Prefs {
            keyboard: Some(keyboard.to_string()),
            layout: Some(layout.to_string()),
            exercise: Some(exercise.to_string()),
        };
        let json = match serde_json::to_string_pretty(&prefs) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("keywiz: could not serialize prefs: {e}");
                return;
            }
        };
        // Atomic: temp file + rename.
        let tmp = path.with_extension("json.tmp");
        if let Err(e) = std::fs::write(&tmp, &json) {
            eprintln!("keywiz: could not write {tmp:?}: {e}");
            return;
        }
        if let Err(e) = std::fs::rename(&tmp, &path) {
            eprintln!("keywiz: could not rename {tmp:?} -> {path:?}: {e}");
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

fn prefs_path() -> Option<PathBuf> {
    let mut path = dirs::data_dir()?;
    path.push("keywiz");
    path.push("prefs.json");
    Some(path)
}
