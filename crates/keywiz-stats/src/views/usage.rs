//! Per-key usage: how often the user presses each key.
//!
//! A sibling to [`heat`](super::heat). Where heat answers "which
//! keys are hurting me right now?", usage answers "where do my
//! fingers actually live?" — independent of skill, stable across
//! fluency levels, and the measurement-side twin of what drift's
//! analyzers try to predict against the corpus.
//!
//! Counts `expected` chars, not `typed`: the signal is "what the
//! layout asks me to type," not "what I hit." (Misfires would
//! conflate the two, and misfire-heat is a different question
//! better answered by a dedicated view.) Uppercase folds to
//! lowercase so `'A'` and `'a'` share the same entry, matching
//! the heat view's convention.
//!
//! Callers get a normalized `f32` in `0.0..=1.0` — **rank-based**
//! over the keys present in the filter window. Magnitude-free
//! normalization was chosen over linear or log for two reasons:
//!
//! 1. *Volume independence.* The same layout looks the same after
//!    500 keystrokes as after 500,000. Users comparing layouts
//!    don't have to dogfood both to equal-time to compare them.
//! 2. *Shape-of-use signal.* The question "which keys are my
//!    workhorses?" is answered by ranking, not by absolute count.
//!    Two letters pressed 3,000 vs. 2,900 times are workhorses
//!    *together*; the 30-press gap between them is noise.
//!
//! A power curve (`RANK_CURVE`) slightly stretches the top of the
//! gradient so the clearly-most-frequent letters stand out from
//! the merely-common middle, without collapsing the tail.
//!
//! Keys with fewer than [`MIN_PRESSES`] are omitted so accidental
//! single-press noise doesn't tint the overlay. Tune there if the
//! noise floor feels wrong.

use std::collections::HashMap;

use anyhow::Result;

use crate::{EventFilter, EventStore, LayoutHash};

/// Keys pressed fewer than this many times are dropped from the
/// usage map. At ~3 presses a typo could tint a key that the user
/// never meaningfully used; above that it's real signal.
pub const MIN_PRESSES: u32 = 3;

/// Power curve on the rank-normalized position. `> 1.0` concentrates
/// saturation at the top (workhorses stand out, tail compresses);
/// `< 1.0` would do the opposite. Tuned by eyeballing realistic
/// corpora; feel free to tweak.
pub const RANK_CURVE: f32 = 1.5;

/// Compute the per-character usage map for events matching `filter`.
///
/// Values are rank-normalized: the most-pressed key is `1.0`, the
/// least-pressed (above [`MIN_PRESSES`]) is near `0.0`, and
/// everything else spreads across the gradient by its rank
/// position. A [`RANK_CURVE`] power curve makes the top stand out.
pub fn usage_map(store: &dyn EventStore, filter: &EventFilter) -> Result<HashMap<char, f32>> {
    let raw = usage_map_raw(store, filter)?;
    let mut entries: Vec<(char, u32)> = raw
        .into_iter()
        .filter(|(_, n)| *n >= MIN_PRESSES)
        .collect();
    if entries.is_empty() {
        return Ok(HashMap::new());
    }
    // Most-pressed first. Stable secondary order on char for a
    // deterministic tie-break across runs.
    entries.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    if entries.len() == 1 {
        return Ok(HashMap::from([(entries[0].0, 1.0)]));
    }
    let last_idx = (entries.len() - 1) as f32;
    Ok(entries
        .into_iter()
        .enumerate()
        .map(|(i, (c, _))| {
            // Top rank → 1.0, bottom rank → 0.0. Power curve
            // concentrates saturation at the top.
            let rank_pos = 1.0 - (i as f32 / last_idx);
            (c, rank_pos.powf(RANK_CURVE).clamp(0.0, 1.0))
        })
        .collect())
}

/// Raw per-character press counts for events matching `filter`.
/// Useful when you want absolute numbers rather than normalized
/// intensity (e.g. to label a key with "1,247 presses").
pub fn usage_map_raw(
    store: &dyn EventStore,
    filter: &EventFilter,
) -> Result<HashMap<char, u32>> {
    let mut counts: HashMap<char, u32> = HashMap::new();
    for event in store.events(filter)? {
        let event = event?;
        let key = event.expected.to_ascii_lowercase();
        *counts.entry(key).or_insert(0) += 1;
    }
    Ok(counts)
}

/// Convenience: usage scoped to a single layout iteration.
pub fn usage_map_for_layout(
    store: &dyn EventStore,
    layout_hash: &LayoutHash,
) -> Result<HashMap<char, f32>> {
    let filter = EventFilter {
        layout_hash: Some(layout_hash.clone()),
        ..Default::default()
    };
    usage_map(store, &filter)
}
