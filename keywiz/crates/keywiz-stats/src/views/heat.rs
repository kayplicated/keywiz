//! Per-key heat: how much a key has cost the user recently.
//!
//! Heat accumulates when the user misses a key and cools as they
//! correct it. The model is an integer-step rule derived from the
//! original in-memory `StatsTracker`:
//!
//! - **miss**: heat += 1, clamped at [`MAX_HEAT`]
//! - **correct on a hot key**: adds one cooling step; every
//!   [`COOL_COST`] correct presses drops heat by one
//! - **correct on a cold key**: no-op
//!
//! Uppercase letters fold to lowercase so a missed capital heats
//! the visible (lowercase) key on the keyboard widget.
//!
//! This view replays the event stream in order and applies the
//! rule. The result matches what the old incrementally-updated
//! `Stats::heat_for` would have produced for the same sequence.
//!
//! Callers get a normalized `f32` in `0.0..=1.0` — the integer
//! steps are an implementation detail renderers don't need.

use std::collections::HashMap;

use anyhow::Result;

use crate::{EventFilter, EventStore, LayoutHash};

/// Heat is clamped at this integer step. 20 matches the pre-event-
/// stream behavior that users have been looking at; tweak carefully.
pub const MAX_HEAT: u32 = 20;

/// Correct presses needed to drop heat by one step. Flat across all
/// steps: 2 corrects = -1 step regardless of current heat.
pub const COOL_COST: u32 = 2;

/// Compute the per-character heat map for events matching `filter`.
///
/// Only keys with non-zero heat appear in the result. Characters
/// are stored as their lowercase form (so `'A'` and `'a'` share
/// the same entry).
///
/// Events are replayed in the order the store yields them —
/// chronological for both `MemoryStore` and `SqliteStore`. Filter
/// semantics are whatever the store contracts promise; the
/// typical call passes a single `layout_hash` so heat stays
/// scoped to one iteration of one layout.
pub fn heat_map(store: &dyn EventStore, filter: &EventFilter) -> Result<HashMap<char, f32>> {
    let raw = heat_map_raw(store, filter)?;
    Ok(raw
        .into_iter()
        .map(|(c, steps)| (c, (steps as f32 / MAX_HEAT as f32).clamp(0.0, 1.0)))
        .collect())
}

/// Same as [`heat_map`] but returns integer heat steps
/// (`0..=MAX_HEAT`) rather than normalized floats. Callers that
/// run their own weighting curves on top of heat (notably the
/// drill exercise's adaptive picker) use this form to keep their
/// tuning constants in the same integer space the model already
/// thinks in.
///
/// Only keys with non-zero steps appear in the result.
pub fn heat_map_raw(
    store: &dyn EventStore,
    filter: &EventFilter,
) -> Result<HashMap<char, u32>> {
    let mut records: HashMap<char, KeyHeat> = HashMap::new();
    for event in store.events(filter)? {
        let event = event?;
        let key = event.expected.to_ascii_lowercase();
        let record = records.entry(key).or_default();
        record.apply(event.correct);
    }
    Ok(records
        .into_iter()
        .filter(|(_, r)| r.heat > 0)
        .map(|(c, r)| (c, r.heat))
        .collect())
}

/// Convenience: heat scoped to a single layout iteration.
/// Equivalent to calling [`heat_map`] with a filter whose
/// `layout_hash` is set.
pub fn heat_map_for_layout(
    store: &dyn EventStore,
    layout_hash: &LayoutHash,
) -> Result<HashMap<char, f32>> {
    let filter = EventFilter {
        layout_hash: Some(layout_hash.clone()),
        ..Default::default()
    };
    heat_map(store, &filter)
}

#[derive(Default, Debug, Clone, Copy)]
struct KeyHeat {
    heat: u32,
    cooling_progress: u32,
}

impl KeyHeat {
    fn apply(&mut self, correct: bool) {
        if !correct {
            self.heat = (self.heat + 1).min(MAX_HEAT);
            return;
        }
        if self.heat == 0 {
            return;
        }
        self.cooling_progress += 1;
        if self.cooling_progress >= COOL_COST {
            self.heat -= 1;
            self.cooling_progress = 0;
        }
    }

}
