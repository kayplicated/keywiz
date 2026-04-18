//! Shared application context passed to all modes.

use std::collections::HashMap;

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
}
