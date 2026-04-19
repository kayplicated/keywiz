//! The runtime Engine — owns the active keyboard + layout, the
//! catalog, broken-selection state, and input processing.

use std::path::{Path, PathBuf};

use crate::engine::catalog::{
    generic_layout_names, keyboard_path, list_json_stems, load_layout_resolved, next_in,
    pick_first_loadable, prev_in, KEYBOARDS_DIR, LAYOUTS_DIR,
};
use crate::keyboard::{self, Keyboard};
use crate::mapping::Layout;

#[derive(Debug, Clone)]
pub struct LayoutChange {
    pub from: String,
    pub to: String,
}

/// The user cycled to this name but the file wouldn't parse. Kept so
/// the UI can color it red without losing the previously-active state.
#[derive(Debug, Clone)]
pub struct BrokenSelection {
    pub name: String,
    pub reason: String,
}

#[derive(Debug)]
pub enum EngineError {
    UnknownKeyboard(String),
    UnknownLayout(String),
    Broken { name: String, message: String },
    Load(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::UnknownKeyboard(n) => write!(f, "unknown keyboard: {n}"),
            EngineError::UnknownLayout(n) => write!(f, "unknown layout: {n}"),
            EngineError::Broken { name, message } => write!(f, "{name}: {message}"),
            EngineError::Load(msg) => write!(f, "{msg}"),
        }
    }
}

/// The runtime coordinator.
pub struct Engine {
    keyboards_dir: PathBuf,
    layouts_dir: PathBuf,
    keyboards: Vec<String>,
    layouts: Vec<String>,
    current_keyboard: String,
    current_layout: String,
    keyboard: Box<dyn Keyboard>,
    layout: Layout,
    broken_keyboard: Option<BrokenSelection>,
    broken_layout: Option<BrokenSelection>,
}

impl Engine {
    pub fn new() -> Result<Self, EngineError> {
        Self::with_dirs(Path::new(KEYBOARDS_DIR), Path::new(LAYOUTS_DIR))
    }

    pub fn with_dirs(keyboards_dir: &Path, layouts_dir: &Path) -> Result<Self, EngineError> {
        let keyboards = list_json_stems(keyboards_dir);
        let layouts = generic_layout_names(layouts_dir, &keyboards);

        let (initial_keyboard, keyboard) =
            pick_first_loadable(&keyboards, "halcyon_elora_v2", |n| {
                keyboard::load(&keyboard_path(keyboards_dir, n))
            })
            .ok_or_else(|| {
                EngineError::Load(format!(
                    "no loadable keyboards in {}",
                    keyboards_dir.display()
                ))
            })?;

        let (initial_layout, layout) = pick_first_loadable(&layouts, "gallium-v2", |n| {
            load_layout_resolved(layouts_dir, n, &initial_keyboard)
        })
        .ok_or_else(|| {
            EngineError::Load(format!(
                "no loadable layouts in {}",
                layouts_dir.display()
            ))
        })?;

        Ok(Engine {
            keyboards_dir: keyboards_dir.to_path_buf(),
            layouts_dir: layouts_dir.to_path_buf(),
            keyboards,
            layouts,
            current_keyboard: initial_keyboard,
            current_layout: initial_layout,
            keyboard,
            layout,
            broken_keyboard: None,
            broken_layout: None,
        })
    }

    /* --- read accessors --- */

    pub fn keyboard(&self) -> &dyn Keyboard {
        self.keyboard.as_ref()
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
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

    /* --- input processing --- */

    /// Look up which physical key id produces the given character
    /// under the active layout. Used by terminal rendering to
    /// highlight the key the user should press next, and by modes
    /// to resolve typed characters to keyboard positions.
    pub fn id_for_char(&self, ch: char) -> Option<&str> {
        self.layout.id_for_char(ch)
    }

    /* --- setters --- */

    /// Switch to a specific keyboard. On parse failure the active
    /// keyboard is unchanged and the name is marked broken.
    pub fn set_keyboard(&mut self, name: &str) -> Result<(), EngineError> {
        if !self.keyboards.iter().any(|n| n == name) {
            return Err(EngineError::UnknownKeyboard(name.to_string()));
        }
        let keyboard = match keyboard::load(&keyboard_path(&self.keyboards_dir, name)) {
            Ok(k) => k,
            Err(reason) => {
                self.broken_keyboard = Some(BrokenSelection {
                    name: name.to_string(),
                    reason: reason.clone(),
                });
                self.current_keyboard = name.to_string();
                return Err(EngineError::Broken {
                    name: name.to_string(),
                    message: reason,
                });
            }
        };
        let layout = match load_layout_resolved(&self.layouts_dir, &self.current_layout, name) {
            Ok(l) => l,
            Err(reason) => {
                self.broken_keyboard = Some(BrokenSelection {
                    name: name.to_string(),
                    reason: reason.clone(),
                });
                self.current_keyboard = name.to_string();
                return Err(EngineError::Broken {
                    name: name.to_string(),
                    message: reason,
                });
            }
        };
        self.broken_keyboard = None;
        self.current_keyboard = name.to_string();
        self.keyboard = keyboard;
        self.layout = layout;
        Ok(())
    }

    pub fn set_layout(&mut self, name: &str) -> Result<LayoutChange, EngineError> {
        if !self.layouts.iter().any(|n| n == name) {
            return Err(EngineError::UnknownLayout(name.to_string()));
        }
        let keyboard = match keyboard::load(&keyboard_path(
            &self.keyboards_dir,
            &self.current_keyboard,
        )) {
            Ok(k) => k,
            Err(message) => return Err(EngineError::Load(message)),
        };
        let layout = match load_layout_resolved(&self.layouts_dir, name, &self.current_keyboard) {
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
                return Err(EngineError::Broken {
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
        self.keyboard = keyboard;
        self.layout = layout;
        Ok(change)
    }

    /* --- cycling --- */

    pub fn next_keyboard(&mut self) -> Result<(), EngineError> {
        let Some(name) = next_in(&self.keyboards, &self.current_keyboard) else {
            return Ok(());
        };
        self.set_keyboard(&name)
    }

    pub fn prev_keyboard(&mut self) -> Result<(), EngineError> {
        let Some(name) = prev_in(&self.keyboards, &self.current_keyboard) else {
            return Ok(());
        };
        self.set_keyboard(&name)
    }

    pub fn next_layout(&mut self) -> Result<LayoutChange, EngineError> {
        let name = next_in(&self.layouts, &self.current_layout)
            .ok_or_else(|| EngineError::UnknownLayout(self.current_layout.clone()))?;
        self.set_layout(&name)
    }

    pub fn prev_layout(&mut self) -> Result<LayoutChange, EngineError> {
        let name = prev_in(&self.layouts, &self.current_layout)
            .ok_or_else(|| EngineError::UnknownLayout(self.current_layout.clone()))?;
        self.set_layout(&name)
    }
}
