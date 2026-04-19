//! Shared application context passed to all modes.

use crate::engine::drill::{CharSource, DrillLevel};
use crate::grid::{Grid, GridManager};
use crate::stats::StatsTracker;
use crate::translate::Translator;

/// Shared state that crosses mode boundaries.
pub struct AppContext {
    pub(crate) show_keyboard: bool,
    /// When true, the keyboard widget tints keys by accuracy instead of finger color.
    pub(crate) show_heatmap: bool,
    /// Input character translator. [`Translator::identity`] when the input
    /// keyboard already matches the target layout.
    pub(crate) translator: Translator,
    /// Name of the layout the physical keyboard actually produces
    /// (`--from`). Kept so the translator can be rebuilt against the
    /// current target whenever the user cycles keyboard or layout.
    /// `None` when no `--from` was given.
    pub(crate) from_layout: Option<String>,
    /// Session + persistent per-key stats for the current layout.
    pub(crate) stats: StatsTracker,
    /// Owns the active keyboard + layout grid. Always present; the kanata
    /// path uses [`GridManager::single`] when there's no catalog to cycle.
    pub(crate) grid_manager: GridManager,
}

impl AppContext {
    pub fn new(
        grid_manager: GridManager,
        translator: Translator,
        from_layout: Option<String>,
    ) -> Self {
        Self {
            show_keyboard: true,
            show_heatmap: false,
            translator,
            from_layout,
            stats: StatsTracker::new(),
            grid_manager,
        }
    }

    /// Translate input character through the active translator.
    pub fn translate_input(&self, ch: char) -> char {
        self.translator.translate(ch)
    }

    /// Active grid (keyboard + layout).
    pub fn grid(&self) -> &Grid {
        self.grid_manager.grid()
    }

    /// Name for stats persistence: the active layout's name.
    pub fn stats_key(&self) -> &str {
        self.grid_manager.current_layout()
    }
}

impl CharSource for AppContext {
    fn chars_for(&self, level: DrillLevel) -> Vec<char> {
        let g = self.grid();
        match level {
            DrillLevel::HomeRow => g.home_row_chars(),
            DrillLevel::HomeAndTop => g.home_and_top_chars(),
            DrillLevel::AllRows => g.all_alpha_chars(),
        }
    }
}
