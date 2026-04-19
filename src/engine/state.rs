//! The runtime Engine — owns the active keyboard, layout, exercise,
//! stats, and display state. One coordinator the rest of the app
//! talks to.

use std::path::{Path, PathBuf};

use crate::engine::catalog::{
    generic_layout_names, keyboard_path, list_json_stems, load_layout_resolved, next_in,
    pick_first_loadable, prev_in, KEYBOARDS_DIR, LAYOUTS_DIR,
};
use crate::engine::placement::{BrokenDisplay, DisplayState, Placement};
use crate::engine::projector::project_for_terminal;
use crate::engine::translate::{self, Translator};
use crate::exercise::{catalog as exercise_catalog, Exercise};
use crate::keyboard::{self, Keyboard};
use crate::mapping::Layout;
use crate::stats::{self, StatsTracker};

#[derive(Debug, Clone)]
pub struct LayoutChange {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone)]
pub struct BrokenSelection {
    pub name: String,
    pub reason: String,
}

impl From<&BrokenSelection> for BrokenDisplay {
    fn from(b: &BrokenSelection) -> Self {
        BrokenDisplay {
            name: b.name.clone(),
            reason: b.reason.clone(),
        }
    }
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

/// Outcome of processing one input char.
#[derive(Debug, Clone, Copy)]
pub struct KeystrokeResult {
    pub hit: bool,
    /// Whether the active exercise is now done.
    pub exercise_done: bool,
}

pub struct Engine {
    keyboards_dir: PathBuf,
    layouts_dir: PathBuf,
    keyboards: Vec<String>,
    layouts: Vec<String>,
    current_keyboard: String,
    current_layout: String,
    current_exercise: String,
    keyboard: Box<dyn Keyboard>,
    layout: Layout,
    exercise: Box<dyn Exercise>,
    translator: Translator,
    from_layout: Option<String>,
    stats: StatsTracker,
    broken_keyboard: Option<BrokenSelection>,
    broken_layout: Option<BrokenSelection>,
    keyboard_visible: bool,
    heatmap_visible: bool,
}

impl Engine {
    pub fn new(from_layout: Option<String>) -> Result<Self, EngineError> {
        Self::with_dirs(
            Path::new(KEYBOARDS_DIR),
            Path::new(LAYOUTS_DIR),
            from_layout,
        )
    }

    pub fn with_dirs(
        keyboards_dir: &Path,
        layouts_dir: &Path,
        from_layout: Option<String>,
    ) -> Result<Self, EngineError> {
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

        let exercise_name = "drill-home".to_string();
        let exercise = exercise_catalog::build(&exercise_name, keyboard.as_ref(), &layout);

        let translator = translate::build(&layout, from_layout.as_deref());
        let mut stats = StatsTracker::new();
        stats.set_persistent(stats::persist::load(&initial_layout));

        Ok(Engine {
            keyboards_dir: keyboards_dir.to_path_buf(),
            layouts_dir: layouts_dir.to_path_buf(),
            keyboards,
            layouts,
            current_keyboard: initial_keyboard,
            current_layout: initial_layout,
            current_exercise: exercise_name,
            keyboard,
            layout,
            exercise,
            translator,
            from_layout,
            stats,
            broken_keyboard: None,
            broken_layout: None,
            keyboard_visible: true,
            heatmap_visible: false,
        })
    }

    /* --- read accessors --- */

    pub fn current_keyboard(&self) -> &str {
        &self.current_keyboard
    }

    pub fn current_layout(&self) -> &str {
        &self.current_layout
    }

    pub fn current_exercise(&self) -> &str {
        &self.current_exercise
    }

    pub fn exercise(&self) -> &dyn Exercise {
        self.exercise.as_ref()
    }

    pub fn exercise_mut(&mut self) -> &mut dyn Exercise {
        self.exercise.as_mut()
    }

    /* --- projection methods --- */

    /// Placements for terminal rendering (pos_a=c, pos_b=r).
    pub fn placements_for_terminal(&self) -> Vec<Placement> {
        project_for_terminal(
            self.keyboard.as_ref(),
            &self.layout,
            self.stats.persistent(),
        )
    }

    /// Build the full DisplayState for a render.
    pub fn display_state(&self) -> DisplayState {
        let mut display = DisplayState::default();
        display.keyboard_short = self.keyboard.short().to_string();
        display.layout_short = self.layout.short.clone();
        display.exercise_short = self.exercise.short().to_string();
        display.broken_keyboard = self.broken_keyboard.as_ref().map(Into::into);
        display.broken_layout = self.broken_layout.as_ref().map(Into::into);
        display.keyboard_visible = self.keyboard_visible;
        display.heatmap_visible = self.heatmap_visible;

        let session = self.stats.session();
        display.session_accuracy = session.overall_accuracy();
        display.session_total_correct = session.total_correct();
        display.session_total_wrong = session.total_wrong();

        self.exercise.fill_display(&mut display);
        display
    }

    /* --- input --- */

    /// Translate + evaluate + record + advance exercise.
    pub fn process_input(&mut self, ch: char) -> KeystrokeResult {
        let translated = self.translator.translate(ch);
        let Some(expected) = self.exercise.expected() else {
            return KeystrokeResult {
                hit: false,
                exercise_done: self.exercise.is_done(),
            };
        };
        let hit = translated == expected;
        self.stats.record(expected, hit);
        if hit {
            self.exercise.advance();
        }
        KeystrokeResult {
            hit,
            exercise_done: self.exercise.is_done(),
        }
    }

    /// Forward a control key (arrows, etc.) to the active exercise.
    /// Returns whether the exercise handled it.
    pub fn handle_exercise_control(&mut self, key: crossterm::event::KeyEvent) -> bool {
        self.exercise.handle_control(key)
    }

    /* --- display toggles --- */

    pub fn toggle_keyboard_visible(&mut self) {
        self.keyboard_visible = !self.keyboard_visible;
    }

    pub fn toggle_heatmap(&mut self) {
        self.heatmap_visible = !self.heatmap_visible;
    }

    /* --- setters (keyboard / layout / exercise) --- */

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
        self.rebuild_exercise();
        self.rebuild_translator();
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
        // Persist outgoing layout's stats before swapping.
        stats::persist::save(&self.current_layout, self.stats.persistent());
        let change = LayoutChange {
            from: std::mem::replace(&mut self.current_layout, name.to_string()),
            to: name.to_string(),
        };
        self.keyboard = keyboard;
        self.layout = layout;
        self.stats = StatsTracker::new();
        self.stats
            .set_persistent(stats::persist::load(&self.current_layout));
        self.rebuild_exercise();
        self.rebuild_translator();
        Ok(change)
    }

    pub fn set_exercise(&mut self, name: &str) {
        self.current_exercise = name.to_string();
        self.rebuild_exercise();
    }

    fn rebuild_exercise(&mut self) {
        self.exercise =
            exercise_catalog::build(&self.current_exercise, self.keyboard.as_ref(), &self.layout);
    }

    fn rebuild_translator(&mut self) {
        self.translator = translate::build(&self.layout, self.from_layout.as_deref());
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

    pub fn next_exercise(&mut self) {
        let name = exercise_catalog::next(&self.current_exercise);
        self.set_exercise(name);
    }

    pub fn prev_exercise(&mut self) {
        let name = exercise_catalog::prev(&self.current_exercise);
        self.set_exercise(name);
    }

    /* --- persistence --- */

    /// Save the active layout's persistent stats to disk. Call on
    /// session boundaries (app exit, exercise switch, etc.).
    pub fn persist_stats(&self) {
        stats::persist::save(&self.current_layout, self.stats.persistent());
    }
}
