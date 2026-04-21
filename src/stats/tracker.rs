//! Two-layer stats bookkeeping.
//!
//! [`StatsTracker`] holds a persistent [`Stats`] (lifetime-of-layout) and a
//! session [`Stats`] (reset when a mode starts or resets). Engines call
//! [`StatsTracker::record`] once per keystroke and both layers are updated.
//!
//! This split lets modes show per-session numbers (current WPM, this run's
//! accuracy) while the persistent layer accumulates data across sessions for
//! features like the keyboard heatmap and auto-generated drill sets.

use super::Stats;

/// Orchestrates session-scoped and persistent stats.
///
/// Records go to both layers. Reading each layer is explicit via
/// [`session`](Self::session) and [`persistent`](Self::persistent).
#[derive(Debug, Clone, Default)]
pub struct StatsTracker {
    session: Stats,
    persistent: Stats,
}

impl StatsTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a keystroke into both layers.
    pub fn record(&mut self, expected: char, correct: bool) {
        self.session.record(expected, correct);
        self.persistent.record(expected, correct);
    }

    /// Stats for the current session only.
    pub fn session(&self) -> &Stats {
        &self.session
    }

    /// Lifetime-of-layout stats.
    pub fn persistent(&self) -> &Stats {
        &self.persistent
    }

    /// Replace the persistent layer wholesale. Used when loading from disk.
    pub fn set_persistent(&mut self, stats: Stats) {
        self.persistent = stats;
    }
}

