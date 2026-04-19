//! File-system catalog of keyboards and layouts.
//!
//! Scans `keyboards/` and `layouts/` directories, lists available
//! names, resolves hardware-specific layout variants
//! (`{layout}-{keyboard}.json` overrides the generic).
//!
//! Kept separate from the engine's state logic so each can evolve
//! independently — catalog is purely about disk discovery.

use std::path::{Path, PathBuf};

use crate::mapping::{self, Layout};

pub const KEYBOARDS_DIR: &str = "keyboards";
pub const LAYOUTS_DIR: &str = "layouts";

pub fn keyboard_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

fn layout_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

/// Load a layout, preferring `{layout}-{keyboard}.json` (hardware
/// variant) over the generic `{layout}.json`.
pub fn load_layout_resolved(
    layouts_dir: &Path,
    layout: &str,
    keyboard: &str,
) -> Result<Layout, String> {
    let variant = layout_path(layouts_dir, &format!("{layout}-{keyboard}"));
    if variant.exists() {
        return mapping::loader::load(&variant);
    }
    let generic = layout_path(layouts_dir, layout);
    mapping::loader::load(&generic)
}

/// Sorted list of JSON file stems in `dir`.
pub fn list_json_stems(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let path = e.path();
                if path.extension().is_some_and(|x| x == "json") {
                    path.file_stem().and_then(|s| s.to_str()).map(String::from)
                } else {
                    None
                }
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    out.sort();
    out
}

/// List *generic* layout names — excludes hardware-variant files
/// (`{layout}-{keyboard}.json`). Variants only surface through
/// resolution, never as standalone entries.
pub fn generic_layout_names(dir: &Path, keyboards: &[String]) -> Vec<String> {
    let mut out: Vec<String> = list_json_stems(dir)
        .into_iter()
        .filter(|name| !is_variant(name, keyboards))
        .collect();
    out.sort();
    out
}

fn is_variant(name: &str, keyboards: &[String]) -> bool {
    keyboards.iter().any(|kb| {
        name.len() > kb.len() + 1
            && name.ends_with(kb.as_str())
            && name.as_bytes()[name.len() - kb.len() - 1] == b'-'
    })
}

/// Walk `list` starting from `preferred` (or first) and return the
/// first name whose loader succeeds.
pub fn pick_first_loadable<T, F>(
    list: &[String],
    preferred: &str,
    mut load: F,
) -> Option<(String, T)>
where
    F: FnMut(&str) -> Result<T, String>,
{
    let start = list.iter().position(|n| n == preferred).unwrap_or(0);
    for i in 0..list.len() {
        let idx = (start + i) % list.len();
        let name = &list[idx];
        match load(name) {
            Ok(value) => return Some((name.clone(), value)),
            Err(message) => {
                eprintln!("keywiz: skipping '{name}': {message}");
            }
        }
    }
    None
}

pub fn next_in(list: &[String], current: &str) -> Option<String> {
    let idx = list.iter().position(|n| n == current).unwrap_or(0);
    list.get((idx + 1) % list.len()).cloned()
}

pub fn prev_in(list: &[String], current: &str) -> Option<String> {
    let idx = list.iter().position(|n| n == current).unwrap_or(0);
    let prev = if idx == 0 { list.len() - 1 } else { idx - 1 };
    list.get(prev).cloned()
}
