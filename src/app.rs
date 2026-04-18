//! Shared application context passed to all modes.

use std::collections::HashMap;

use crate::layout::Layout;
use crate::stats::StatsTracker;

/// Shared state that crosses mode boundaries.
pub struct AppContext {
    pub(crate) layout: Layout,
    pub(crate) split: bool,
    pub(crate) show_keyboard: bool,
    /// Input translation map: physical key -> target layout key.
    /// None means input is already in the target layout.
    pub(crate) translate: Option<HashMap<char, char>>,
    /// Session + persistent per-key stats for the current layout.
    pub(crate) stats: StatsTracker,
}

impl AppContext {
    pub fn new(layout: Layout, split: bool, translate: Option<HashMap<char, char>>) -> Self {
        Self {
            layout,
            split,
            show_keyboard: true,
            translate,
            stats: StatsTracker::new(),
        }
    }

    /// Translate input character if translation is active.
    pub fn translate_input(&self, ch: char) -> char {
        match &self.translate {
            Some(map) => map.get(&ch).copied().unwrap_or(ch),
            None => ch,
        }
    }
}
