//! Runtime keyboard + layout selection.
//!
//! Scans `keyboards/` and `layouts/` at startup to build a catalog.
//! Broken JSONs stay in the catalog marked as broken — cycling to them
//! shows them in red in the UI but keeps the previously-applied grid
//! active. This way a user can see that a file exists but refuses to
//! load, without the app crashing or silently hiding the problem.
//!
//! Resolution rule: when applying layout "X" to keyboard "Y", the
//! manager prefers `{X}-{Y}.json` (hardware-specific override) and
//! falls back to `{X}.json` (generic). Variants only surface through
//! resolution, never as standalone entries in the layout list.

use std::path::{Path, PathBuf};

use crate::configreader::{keyboard as kb_reader, layout as layout_reader};
use crate::physical::PhysicalKeyboard;

use super::{Grid, Layout};

const KEYBOARDS_DIR: &str = "keyboards";
const LAYOUTS_DIR: &str = "layouts";

#[derive(Debug, Clone)]
pub struct LayoutChange {
    pub from: String,
    pub to: String,
}

/// Error from a manager operation. `Broken` means the selection
/// succeeded in the sense that the name exists in the catalog, but the
/// file failed to parse — the active grid is unchanged and the caller
/// can flag the broken name to the user.
#[derive(Debug)]
pub enum GridError {
    UnknownKeyboard(String),
    UnknownLayout(String),
    Broken { name: String, message: String },
    Load(String),
}

impl std::fmt::Display for GridError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GridError::UnknownKeyboard(n) => write!(f, "unknown keyboard: {n}"),
            GridError::UnknownLayout(n) => write!(f, "unknown layout: {n}"),
            GridError::Broken { name, message } => write!(f, "{name}: {message}"),
            GridError::Load(msg) => write!(f, "{msg}"),
        }
    }
}

/// Owns the catalog of keyboards/layouts and the currently active grid.
pub struct GridManager {
    keyboards_dir: PathBuf,
    layouts_dir: PathBuf,
    keyboards: Vec<String>,
    layouts: Vec<String>,
    current_keyboard: String,
    current_layout: String,
    grid: Grid,
    /// Names cycled past that failed to load on last attempt. Kept so the
    /// UI can color the footer red when the user is "on" a broken name.
    broken_keyboard: Option<BrokenSelection>,
    broken_layout: Option<BrokenSelection>,
}

/// A broken selection — the user cycled to this name but the file
/// wouldn't parse. The UI renders the name in red and can optionally
/// show the reason (truncated) so the user can fix the file.
#[derive(Debug, Clone)]
pub struct BrokenSelection {
    pub name: String,
    pub reason: String,
}

impl GridManager {
    pub fn new() -> Result<Self, GridError> {
        Self::with_dirs(Path::new(KEYBOARDS_DIR), Path::new(LAYOUTS_DIR))
    }

    /// Build a manager that owns a single externally-supplied grid (e.g.
    /// from a config reader). No catalog; cycling is a no-op.
    pub fn single(grid: Grid) -> Self {
        let keyboard = grid.keyboard_name.clone();
        let layout = grid.layout_name.clone();
        GridManager {
            keyboards_dir: PathBuf::new(),
            layouts_dir: PathBuf::new(),
            keyboards: vec![keyboard.clone()],
            layouts: vec![layout.clone()],
            current_keyboard: keyboard,
            current_layout: layout,
            grid,
            broken_keyboard: None,
            broken_layout: None,
        }
    }

    pub fn with_dirs(keyboards_dir: &Path, layouts_dir: &Path) -> Result<Self, GridError> {
        let keyboards = list_json_stems(keyboards_dir);
        let layouts = generic_layout_names(layouts_dir, &keyboards);

        // Find the first keyboard that actually loads, falling back
        // through the catalog. If nothing loads we can't start.
        let (initial_keyboard, keyboard) = pick_first_loadable(&keyboards, "halcyon_elora_v2", |n| {
            kb_reader::load(&keyboard_path(keyboards_dir, n))
        })
        .ok_or_else(|| {
            GridError::Load(format!("no loadable keyboards in {}", keyboards_dir.display()))
        })?;

        // Same for layouts, resolved against the chosen keyboard.
        let (initial_layout, layout) = pick_first_loadable(&layouts, "gallium-v2", |n| {
            load_resolved(layouts_dir, n, &initial_keyboard)
        })
        .ok_or_else(|| GridError::Load(format!("no loadable layouts in {}", layouts_dir.display())))?;

        let grid = Grid::compose(&keyboard, &layout);

        Ok(GridManager {
            keyboards_dir: keyboards_dir.to_path_buf(),
            layouts_dir: layouts_dir.to_path_buf(),
            keyboards,
            layouts,
            current_keyboard: initial_keyboard,
            current_layout: initial_layout,
            grid,
            broken_keyboard: None,
            broken_layout: None,
        })
    }

    /* --- read accessors --- */

    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    pub fn current_keyboard(&self) -> &str {
        &self.current_keyboard
    }

    pub fn current_layout(&self) -> &str {
        &self.current_layout
    }

    pub fn broken_keyboard(&self) -> Option<&BrokenSelection> {
        self.broken_keyboard.as_ref()
    }

    pub fn broken_layout(&self) -> Option<&BrokenSelection> {
        self.broken_layout.as_ref()
    }

    /* --- setters --- */

    /// Switch to a specific keyboard by name. On parse failure the
    /// active grid is unchanged and the keyboard is marked broken so
    /// the UI can show the failure without losing the working state.
    pub fn set_keyboard(&mut self, name: &str) -> Result<(), GridError> {
        if !self.keyboards.iter().any(|n| n == name) {
            return Err(GridError::UnknownKeyboard(name.to_string()));
        }
        let keyboard = match kb_reader::load(&keyboard_path(&self.keyboards_dir, name)) {
            Ok(k) => k,
            Err(reason) => {
                self.broken_keyboard = Some(BrokenSelection {
                    name: name.to_string(),
                    reason: reason.clone(),
                });
                self.current_keyboard = name.to_string();
                return Err(GridError::Broken {
                    name: name.to_string(),
                    message: reason,
                });
            }
        };
        let layout = match load_resolved(&self.layouts_dir, &self.current_layout, name) {
            Ok(l) => l,
            Err(reason) => {
                self.broken_keyboard = Some(BrokenSelection {
                    name: name.to_string(),
                    reason: reason.clone(),
                });
                self.current_keyboard = name.to_string();
                return Err(GridError::Broken {
                    name: name.to_string(),
                    message: reason,
                });
            }
        };
        self.broken_keyboard = None;
        self.current_keyboard = name.to_string();
        self.grid = Grid::compose(&keyboard, &layout);
        Ok(())
    }

    pub fn set_layout(&mut self, name: &str) -> Result<LayoutChange, GridError> {
        if !self.layouts.iter().any(|n| n == name) {
            return Err(GridError::UnknownLayout(name.to_string()));
        }
        let keyboard = match kb_reader::load(&keyboard_path(&self.keyboards_dir, &self.current_keyboard)) {
            Ok(k) => k,
            Err(message) => return Err(GridError::Load(message)),
        };
        let layout = match load_resolved(&self.layouts_dir, name, &self.current_keyboard) {
            Ok(l) => l,
            Err(reason) => {
                self.broken_layout = Some(BrokenSelection {
                    name: name.to_string(),
                    reason: reason.clone(),
                });
                let change = LayoutChange {
                    from: std::mem::replace(&mut self.current_layout, name.to_string()),
                    to: name.to_string(),
                };
                return Err(GridError::Broken {
                    name: name.to_string(),
                    message: format!("{reason} (from: {})", change.from),
                });
            }
        };
        self.broken_layout = None;
        let change = LayoutChange {
            from: std::mem::replace(&mut self.current_layout, name.to_string()),
            to: name.to_string(),
        };
        self.grid = Grid::compose(&keyboard, &layout);
        Ok(change)
    }

    /* --- cycling --- */

    pub fn next_keyboard(&mut self) -> Result<(), GridError> {
        let Some(name) = next_in(&self.keyboards, &self.current_keyboard) else {
            return Ok(());
        };
        self.set_keyboard(&name)
    }

    pub fn prev_keyboard(&mut self) -> Result<(), GridError> {
        let Some(name) = prev_in(&self.keyboards, &self.current_keyboard) else {
            return Ok(());
        };
        self.set_keyboard(&name)
    }

    pub fn next_layout(&mut self) -> Result<LayoutChange, GridError> {
        let name = next_in(&self.layouts, &self.current_layout)
            .ok_or_else(|| GridError::UnknownLayout(self.current_layout.clone()))?;
        self.set_layout(&name)
    }

    pub fn prev_layout(&mut self) -> Result<LayoutChange, GridError> {
        let name = prev_in(&self.layouts, &self.current_layout)
            .ok_or_else(|| GridError::UnknownLayout(self.current_layout.clone()))?;
        self.set_layout(&name)
    }
}

/* --- file system helpers --- */

fn keyboard_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

fn layout_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.json"))
}

fn load_resolved(layouts_dir: &Path, layout: &str, keyboard: &str) -> Result<Layout, String> {
    let variant = layout_path(layouts_dir, &format!("{layout}-{keyboard}"));
    if variant.exists() {
        return layout_reader::load(&variant);
    }
    let generic = layout_path(layouts_dir, layout);
    layout_reader::load(&generic)
}

fn list_json_stems(dir: &Path) -> Vec<String> {
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

fn generic_layout_names(dir: &Path, keyboards: &[String]) -> Vec<String> {
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

/// Walk the list starting from `preferred` (or the first entry) and
/// return the first name whose load function succeeds.
fn pick_first_loadable<T, F>(list: &[String], preferred: &str, mut load: F) -> Option<(String, T)>
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

fn next_in(list: &[String], current: &str) -> Option<String> {
    let idx = list.iter().position(|n| n == current).unwrap_or(0);
    list.get((idx + 1) % list.len()).cloned()
}

fn prev_in(list: &[String], current: &str) -> Option<String> {
    let idx = list.iter().position(|n| n == current).unwrap_or(0);
    let prev = if idx == 0 { list.len() - 1 } else { idx - 1 };
    list.get(prev).cloned()
}
