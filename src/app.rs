//! Shared application context passed to all modes.

use std::collections::HashMap;

use crate::engine::drill::{CharSource, DrillLevel};
use crate::grid::GridManager;
use crate::layout::Layout;
use crate::stats::StatsTracker;

/// Shared state that crosses mode boundaries.
pub struct AppContext {
    pub(crate) layout: Layout,
    pub(crate) split: bool,
    pub(crate) show_keyboard: bool,
    /// When true, the keyboard widget tints keys by accuracy instead of finger color.
    pub(crate) show_heatmap: bool,
    /// Input translation map: physical key -> target layout key.
    /// None means input is already in the target layout.
    pub(crate) translate: Option<HashMap<char, char>>,
    /// Session + persistent per-key stats for the current layout.
    pub(crate) stats: StatsTracker,
    /// Optional data-driven grid manager. When present, modes and widgets
    /// that know about it will prefer it over the legacy `layout` field.
    /// Absent when keywiz was started via the kanata path.
    pub(crate) grid_manager: Option<GridManager>,
}

impl AppContext {
    pub fn new(layout: Layout, split: bool, translate: Option<HashMap<char, char>>) -> Self {
        Self {
            layout,
            split,
            show_keyboard: true,
            show_heatmap: false,
            translate,
            stats: StatsTracker::new(),
            grid_manager: None,
        }
    }

    /// Replace the grid manager (used when booting via the data-driven path).
    pub fn with_grid_manager(mut self, manager: GridManager) -> Self {
        self.grid_manager = Some(manager);
        self
    }

    /// Translate input character if translation is active.
    pub fn translate_input(&self, ch: char) -> char {
        match &self.translate {
            Some(map) => map.get(&ch).copied().unwrap_or(ch),
            None => ch,
        }
    }

    /// Alphabetic characters on the home row of the active source.
    /// Prefers the grid manager when present, falling back to the legacy
    /// [`Layout`].
    pub fn home_row_chars(&self) -> Vec<char> {
        match &self.grid_manager {
            Some(m) => m.grid().home_row_chars(),
            None => self.layout.home_row_chars(),
        }
    }

    /// Home row plus the row above it.
    pub fn home_and_top_chars(&self) -> Vec<char> {
        match &self.grid_manager {
            Some(m) => m.grid().home_and_top_chars(),
            None => {
                // Legacy: home + top row (row index 1 = top row).
                let mut c = self.layout.home_row_chars();
                c.extend(
                    self.layout.rows[1]
                        .keys
                        .iter()
                        .map(|k| k.lower)
                        .filter(|c| c.is_alphabetic()),
                );
                c
            }
        }
    }

    /// All alphabetic characters produced by the active source.
    pub fn all_chars(&self) -> Vec<char> {
        match &self.grid_manager {
            Some(m) => m.grid().all_alpha_chars(),
            None => self.layout.all_chars(),
        }
    }

    /// Name for stats persistence: grid layout name if present,
    /// otherwise the legacy layout name.
    pub fn stats_key(&self) -> &str {
        match &self.grid_manager {
            Some(m) => m.current_layout(),
            None => &self.layout.name,
        }
    }
}

impl CharSource for AppContext {
    fn chars_for(&self, level: DrillLevel) -> Vec<char> {
        match level {
            DrillLevel::HomeRow => self.home_row_chars(),
            DrillLevel::HomeAndTop => self.home_and_top_chars(),
            DrillLevel::AllRows => self.all_chars(),
        }
    }
}
