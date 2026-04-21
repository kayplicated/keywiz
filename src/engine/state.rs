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

/// Outcome of processing one input char. Staged for metrics / stats
/// hooks — today main.rs discards the return value, but the fields
/// are the obvious seam for per-hit side effects (sound, haptics,
/// session timers, end-of-exercise dispatch).
#[allow(dead_code)]
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
    /// Active exercise category (`drill`, `words`, `text`).
    current_category: String,
    /// Active instance index within the current category.
    current_instance: usize,
    /// Per-category memory of the last instance visited. Lets users
    /// cycle away from text passage 7 to drill and back without
    /// losing their place.
    instance_memory: std::collections::HashMap<String, usize>,
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

        let translator = translate::build(&layout, from_layout.as_deref());
        let mut stats = StatsTracker::new();
        stats.set_persistent(stats::persist::load(&initial_layout));

        // Exercise depends on stats for heat-aware construction
        // (drill's starting level), so stats must be loaded first.
        let current_category = "drill".to_string();
        let current_instance = 0;
        let exercise = exercise_catalog::build(
            &current_category,
            current_instance,
            keyboard.as_ref(),
            &layout,
            stats.persistent(),
        );

        Ok(Engine {
            keyboards_dir: keyboards_dir.to_path_buf(),
            layouts_dir: layouts_dir.to_path_buf(),
            keyboards,
            layouts,
            current_keyboard: initial_keyboard,
            current_layout: initial_layout,
            current_category,
            current_instance,
            instance_memory: std::collections::HashMap::new(),
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

    /// Serialized form of the active exercise for prefs
    /// persistence. See `exercise::catalog::format_pref`.
    pub fn current_exercise(&self) -> String {
        exercise_catalog::format_pref(&self.current_category, self.current_instance)
    }

    /// Number of instances in the current category (0 for drill).
    pub fn current_instance_count(&self) -> usize {
        exercise_catalog::instance_count(&self.current_category)
    }

    /// Human label for the current instance, e.g. `"50"`,
    /// `"Endless"`, `"The Commit"`. `None` when the category has
    /// no instance axis.
    pub fn current_instance_label(&self) -> Option<String> {
        exercise_catalog::instance_label(&self.current_category, self.current_instance)
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
        let instance_count = self.current_instance_count();
        display.exercise_instance = if instance_count == 0 {
            (0, 0)
        } else {
            (self.current_instance + 1, instance_count)
        };
        display.exercise_instance_label = self.current_instance_label();
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
        // Pass both hit and miss to the exercise — the drill's
        // autoscaler needs both signals for its rolling window.
        self.exercise.advance(self.stats.persistent(), hit);
        KeystrokeResult {
            hit,
            exercise_done: self.exercise.is_done(),
        }
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

    /// Set the active exercise from a serialized prefs string
    /// (`"text:3"`, `"words:50"`, `"drill"`, or any legacy name).
    /// Safe against unknown formats — falls back to drill.
    pub fn set_exercise_from_pref(&mut self, pref: &str) {
        let (cat, inst) = exercise_catalog::parse_pref(pref);
        self.set_category_instance(cat, inst);
    }

    fn set_category_instance(&mut self, category: String, instance: usize) {
        // Remember where we were in the outgoing category before
        // moving on.
        self.instance_memory
            .insert(self.current_category.clone(), self.current_instance);
        self.current_category = category;
        // Clamp instance to the category's range.
        let bound = exercise_catalog::instance_count(&self.current_category);
        self.current_instance = if bound == 0 { 0 } else { instance.min(bound - 1) };
        self.rebuild_exercise();
    }

    fn rebuild_exercise(&mut self) {
        self.exercise = exercise_catalog::build(
            &self.current_category,
            self.current_instance,
            self.keyboard.as_ref(),
            &self.layout,
            self.stats.persistent(),
        );
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

    /// Cycle to the next exercise category (Alt+↓). Restores the
    /// new category's remembered instance if one exists.
    pub fn next_exercise_category(&mut self) {
        let next = exercise_catalog::next_category(&self.current_category).to_string();
        let inst = self.remembered_instance(&next);
        self.set_category_instance(next, inst);
    }

    /// Cycle to the previous exercise category (Alt+↑).
    pub fn prev_exercise_category(&mut self) {
        let prev = exercise_catalog::prev_category(&self.current_category).to_string();
        let inst = self.remembered_instance(&prev);
        self.set_category_instance(prev, inst);
    }

    /// Cycle to the next instance within the current category
    /// (Alt+→). No-op when the category has no instances (drill).
    pub fn next_exercise_instance(&mut self) {
        if let Some(next) =
            exercise_catalog::next_instance(&self.current_category, self.current_instance)
        {
            self.current_instance = next;
            self.rebuild_exercise();
        }
    }

    /// Cycle to the previous instance within the current category
    /// (Alt+←).
    pub fn prev_exercise_instance(&mut self) {
        if let Some(prev) =
            exercise_catalog::prev_instance(&self.current_category, self.current_instance)
        {
            self.current_instance = prev;
            self.rebuild_exercise();
        }
    }

    fn remembered_instance(&self, category: &str) -> usize {
        self.instance_memory.get(category).copied().unwrap_or(0)
    }

    /* --- persistence --- */

    /// Save the active layout's persistent stats to disk. Call on
    /// session boundaries (app exit, exercise switch, etc.).
    pub fn persist_stats(&self) {
        stats::persist::save(&self.current_layout, self.stats.persistent());
    }
}
