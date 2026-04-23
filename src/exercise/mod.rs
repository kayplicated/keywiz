//! Exercises — typing-tutor content types driven by the engine.
//!
//! Each exercise owns its content and progression cursor. The
//! engine asks `expected()` for the next target character,
//! compares against input, calls `advance(heat, correct)` after
//! each keystroke (hit or miss — the drill autoscaler needs both
//! signals), and queries `fill_display()` to populate
//! exercise-specific fields in the `DisplayState` it hands to the
//! renderer.
//!
//! Exercises **read** heat (to bias pick / gate progression) but
//! never **record** — the engine owns the write path to
//! keywiz-stats.

pub mod catalog;
pub mod drill;
pub mod text;
pub mod words;

use std::collections::HashMap;

use crate::engine::placement::DisplayState;

/// Per-char integer heat steps, as produced by
/// `keywiz_stats::views::heat::heat_map_raw`. Values are
/// `1..=MAX_HEAT` (cold keys are absent). Drill runs its own
/// integer math on top of this; exercises that don't care about
/// heat (words, text) simply ignore the map.
pub type HeatSteps = HashMap<char, u32>;

pub trait Exercise {
    /// Staged — canonical name, for metrics/logging. `short()` is
    /// what renders today.
    #[allow(dead_code)]
    fn name(&self) -> &str;
    fn short(&self) -> &str;
    fn expected(&self) -> Option<char>;
    /// Called after every keystroke (hit or miss). `correct = true`
    /// means the user matched `expected()`; `correct = false` means
    /// they missed. `heat` is read-only per-char heat in integer
    /// steps — drill biases its picker on this.
    fn advance(&mut self, heat: &HeatSteps, correct: bool);
    fn is_done(&self) -> bool;
    fn fill_display(&self, display: &mut DisplayState);
}
