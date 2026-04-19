//! Disk persistence for per-layout [`Stats`](super::Stats).
//!
//! Stats live under the OS-appropriate data directory:
//! - Linux: `~/.local/share/keywiz/stats/<layout>.json`
//! - macOS: `~/Library/Application Support/keywiz/stats/<layout>.json`
//! - Windows: `%APPDATA%\keywiz\stats\<layout>.json`
//!
//! All I/O failures are logged to stderr and swallowed — persistence is a
//! quality-of-life feature and must never crash the app or block typing.
//! Saves are atomic: written to a temp file, then renamed into place.

use super::Stats;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Current on-disk schema version. Bump when the format changes in a
/// non-backward-compatible way.
const CURRENT_VERSION: u32 = 1;

/// On-disk representation. The layout name is duplicated from the filename
/// as defense-in-depth against a file being renamed or copied between layouts.
///
/// `#[serde(flatten)]` pulls `Stats`' `keys` field up to the top level, so
/// the JSON reads as `{ "version": 1, "layout": "...", "keys": {...} }`
/// rather than nesting another object.
#[derive(Debug, Serialize, Deserialize)]
struct StatsFile {
    version: u32,
    layout: String,
    #[serde(flatten)]
    stats: Stats,
}

/// Resolve the stats file path for a given layout.
/// Returns `None` if no data directory is available on this system.
fn stats_path(layout: &str) -> Option<PathBuf> {
    let mut path = dirs::data_dir()?;
    path.push("keywiz");
    path.push("stats");
    path.push(format!("{layout}.json"));
    Some(path)
}

/// Load stats for a layout. Returns an empty `Stats` if the file doesn't
/// exist, can't be read, or fails to parse (with a stderr warning in the
/// parse-failure case — a missing file on first run is normal).
pub fn load(layout: &str) -> Stats {
    let Some(path) = stats_path(layout) else {
        return Stats::default();
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Stats::default(),
        Err(e) => {
            eprintln!("keywiz: could not read stats from {path:?}: {e}");
            return Stats::default();
        }
    };

    match serde_json::from_str::<StatsFile>(&content) {
        Ok(file) if file.version == CURRENT_VERSION && file.layout == layout => file.stats,
        Ok(file) if file.layout != layout => {
            eprintln!(
                "keywiz: stats file {path:?} claims layout '{}', expected '{layout}' — ignoring",
                file.layout
            );
            Stats::default()
        }
        Ok(file) => {
            eprintln!(
                "keywiz: stats file {path:?} has unsupported version {} — ignoring",
                file.version
            );
            Stats::default()
        }
        Err(e) => {
            eprintln!("keywiz: could not parse {path:?}: {e}");
            Stats::default()
        }
    }
}

/// Save stats for a layout. Writes atomically via a temp file + rename so
/// a crash mid-save can never produce a corrupt stats file. Failures are
/// logged to stderr and swallowed.
pub fn save(layout: &str, stats: &Stats) {
    let Some(path) = stats_path(layout) else {
        return;
    };

    if let Some(parent) = path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("keywiz: could not create {parent:?}: {e}");
        return;
    }

    let file = StatsFile {
        version: CURRENT_VERSION,
        layout: layout.to_string(),
        stats: stats.clone(),
    };

    let json = match serde_json::to_string_pretty(&file) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("keywiz: could not serialize stats: {e}");
            return;
        }
    };

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

