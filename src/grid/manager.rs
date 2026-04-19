//! Runtime keyboard + layout selection.
//!
//! Scans `keyboards/` and `layouts/` at startup to build a catalog, then
//! keeps one keyboard + one layout active. All mutation flows through
//! this type — CLI flags, future keybinds, a future menu — so there's one
//! source of truth for what's currently loaded.
//!
//! Resolution rule: when applying layout "X" to keyboard "Y", the manager
//! prefers `{X}-{Y}.json` (hardware-specific override) and falls back to
//! `{X}.json` (generic). Variants only surface through resolution, never
//! as standalone entries in the layout list.

use std::path::{Path, PathBuf};

use super::{Grid, Keyboard, Layout};

const KEYBOARDS_DIR: &str = "keyboards";
const LAYOUTS_DIR: &str = "layouts";

/// Emitted by mutating methods so the caller can react — for example, the
/// event loop uses this to swap per-layout persistent stats when the
/// layout changes.
#[derive(Debug, Clone)]
pub struct LayoutChange {
    pub from: String,
    pub to: String,
}

#[derive(Debug)]
pub enum GridError {
    UnknownKeyboard(String),
    UnknownLayout(String),
    Load(String),
}

impl std::fmt::Display for GridError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GridError::UnknownKeyboard(n) => write!(f, "unknown keyboard: {n}"),
            GridError::UnknownLayout(n) => write!(f, "unknown layout: {n}"),
            GridError::Load(msg) => write!(f, "{msg}"),
        }
    }
}

/// Owns the catalog of keyboards/layouts and the currently active grid.
pub struct GridManager {
    keyboards_dir: PathBuf,
    layouts_dir: PathBuf,
    /// Sorted list of available keyboard names.
    keyboards: Vec<String>,
    /// Sorted list of *generic* layout names. Hardware-specific variants
    /// don't appear here — they're resolved transparently by [`set_layout`].
    layouts: Vec<String>,
    current_keyboard: String,
    current_layout: String,
    grid: Grid,
}

impl GridManager {
    /// Build a manager by scanning the default `keyboards/` and `layouts/`
    /// directories and loading initial defaults. If the preferred defaults
    /// aren't available, falls back to whatever's present.
    pub fn new() -> Result<Self, GridError> {
        Self::with_dirs(Path::new(KEYBOARDS_DIR), Path::new(LAYOUTS_DIR))
    }

    /// Build a manager that owns a single externally-supplied grid (e.g.
    /// from a config reader). No catalog of alternates — cycling is a
    /// no-op. Use this when the grid comes from a source other than the
    /// `keyboards/` + `layouts/` directories.
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
        }
    }

    /// Like [`new`] but with explicit directories — used in tests.
    pub fn with_dirs(keyboards_dir: &Path, layouts_dir: &Path) -> Result<Self, GridError> {
        let keyboards = list_json_stems(keyboards_dir);
        let layouts = generic_layout_names(layouts_dir, &keyboards);

        let initial_keyboard = pick_default(&keyboards, "us_intl")
            .ok_or_else(|| GridError::Load(format!("no keyboards in {}", keyboards_dir.display())))?;
        let initial_layout = pick_default(&layouts, "qwerty")
            .ok_or_else(|| GridError::Load(format!("no layouts in {}", layouts_dir.display())))?;

        let keyboard = Keyboard::load(&keyboard_path(keyboards_dir, &initial_keyboard))
            .map_err(GridError::Load)?;
        let layout = load_resolved(layouts_dir, &initial_layout, &initial_keyboard)
            .map_err(GridError::Load)?;
        let grid = Grid::compose(&keyboard, &layout);

        Ok(GridManager {
            keyboards_dir: keyboards_dir.to_path_buf(),
            layouts_dir: layouts_dir.to_path_buf(),
            keyboards,
            layouts,
            current_keyboard: initial_keyboard,
            current_layout: initial_layout,
            grid,
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

    /* --- setters --- */

    /// Switch to a specific keyboard by name. The current layout is
    /// re-resolved against the new keyboard (variant file preferred).
    pub fn set_keyboard(&mut self, name: &str) -> Result<(), GridError> {
        if !self.keyboards.iter().any(|n| n == name) {
            return Err(GridError::UnknownKeyboard(name.to_string()));
        }
        let keyboard = Keyboard::load(&keyboard_path(&self.keyboards_dir, name))
            .map_err(GridError::Load)?;
        let layout = load_resolved(&self.layouts_dir, &self.current_layout, name)
            .map_err(GridError::Load)?;
        self.current_keyboard = name.to_string();
        self.grid = Grid::compose(&keyboard, &layout);
        Ok(())
    }

    /// Switch to a specific layout by name. Returns a [`LayoutChange`] so
    /// callers (notably the event loop) can react — e.g. save/reload
    /// per-layout stats.
    pub fn set_layout(&mut self, name: &str) -> Result<LayoutChange, GridError> {
        if !self.layouts.iter().any(|n| n == name) {
            return Err(GridError::UnknownLayout(name.to_string()));
        }
        let keyboard = Keyboard::load(&keyboard_path(&self.keyboards_dir, &self.current_keyboard))
            .map_err(GridError::Load)?;
        let layout = load_resolved(&self.layouts_dir, name, &self.current_keyboard)
            .map_err(GridError::Load)?;
        let change = LayoutChange {
            from: std::mem::replace(&mut self.current_layout, name.to_string()),
            to: name.to_string(),
        };
        self.grid = Grid::compose(&keyboard, &layout);
        Ok(change)
    }

    /* --- cycling (future keybinds call these) --- */

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

/// Load a layout, preferring a `{layout}-{keyboard}.json` override over
/// the generic file. Variant files only apply when explicitly paired with
/// their keyboard.
fn load_resolved(layouts_dir: &Path, layout: &str, keyboard: &str) -> Result<Layout, String> {
    let variant = layout_path(layouts_dir, &format!("{layout}-{keyboard}"));
    if variant.exists() {
        return Layout::load(&variant);
    }
    let generic = layout_path(layouts_dir, layout);
    Layout::load(&generic)
}

/// Return sorted stems of `*.json` files in `dir`.
fn list_json_stems(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let path = e.path();
                if path.extension().is_some_and(|x| x == "json") {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
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

/// List generic layout names. A file stem is a **variant** (and thus
/// excluded from the generic catalog) only when it ends in `-<keyboard>`
/// where `<keyboard>` is an installed keyboard name. This lets layout
/// names freely contain hyphens (e.g. `colemak-dh`) without being
/// mistaken for variants.
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

fn pick_default(list: &[String], preferred: &str) -> Option<String> {
    if list.iter().any(|n| n == preferred) {
        Some(preferred.to_string())
    } else {
        list.first().cloned()
    }
}

fn next_in(list: &[String], current: &str) -> Option<String> {
    let idx = list.iter().position(|n| n == current)?;
    list.get((idx + 1) % list.len()).cloned()
}

fn prev_in(list: &[String], current: &str) -> Option<String> {
    let idx = list.iter().position(|n| n == current)?;
    let prev = if idx == 0 { list.len() - 1 } else { idx - 1 };
    list.get(prev).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_wraps_around() {
        let list = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(next_in(&list, "a"), Some("b".into()));
        assert_eq!(next_in(&list, "c"), Some("a".into()));
    }

    #[test]
    fn prev_wraps_around() {
        let list = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(prev_in(&list, "a"), Some("c".into()));
        assert_eq!(prev_in(&list, "b"), Some("a".into()));
    }

    /// Smoke test: the files we ship actually load and compose.
    #[test]
    fn ships_valid_us_intl_and_qwerty() {
        let mgr = GridManager::new().expect("shipped files should load");
        assert_eq!(mgr.current_layout(), "qwerty");
        let grid = mgr.grid();
        // Every shipped button should have a mapping in qwerty.
        let unmapped = grid.buttons.iter().filter(|b| b.mapping.is_none()).count();
        assert_eq!(
            unmapped, 0,
            "us_intl has {unmapped} button(s) that qwerty doesn't map"
        );
        // Home row should include 'a' and 'j'.
        let home = grid.home_row_chars();
        assert!(home.contains(&'a'));
        assert!(home.contains(&'j'));
    }

    #[test]
    fn is_variant_requires_matching_keyboard_suffix() {
        let kb = vec!["us_intl".to_string(), "elora".to_string()];
        // Plain generic layouts.
        assert!(!is_variant("qwerty", &kb));
        assert!(!is_variant("colemak", &kb));
        // Hyphen in the name but no keyboard match: not a variant.
        assert!(!is_variant("colemak-dh", &kb));
        // Variant: suffix matches an installed keyboard.
        assert!(is_variant("gallium-elora", &kb));
        assert!(is_variant("qwerty-us_intl", &kb));
    }
}
