//! Exercises — typing-tutor content types driven by the engine.
//!
//! Each exercise owns its content and progression cursor. The
//! engine asks `expected()` for the next target character,
//! compares against input, calls `advance(stats, correct)` after
//! each keystroke (hit or miss — the drill autoscaler needs both
//! signals), and queries `fill_display()` to populate
//! exercise-specific fields in the `DisplayState` it hands to the
//! renderer.
//!
//! Exercises **read** stats (to bias pick / gate progression) but
//! never **record** to them — the engine owns the write path.

pub mod catalog;
pub mod drill;
pub mod text;
pub mod words;

use crate::engine::placement::DisplayState;
use crate::stats::Stats;

pub trait Exercise {
    /// Staged — canonical name, for metrics/logging. `short()` is
    /// what renders today.
    #[allow(dead_code)]
    fn name(&self) -> &str;
    fn short(&self) -> &str;
    fn expected(&self) -> Option<char>;
    /// Called after every keystroke (hit or miss). `correct = true`
    /// means the user matched `expected()`; `correct = false` means
    /// they missed. Exercises use `correct` to advance the cursor
    /// (only on hit, typically) and to update internal progression
    /// state (e.g. drill's rolling-window autoscaler). `stats` is
    /// read-only — the engine has already recorded the keystroke.
    fn advance(&mut self, stats: &Stats, correct: bool);
    fn is_done(&self) -> bool;
    fn fill_display(&self, display: &mut DisplayState);
}
