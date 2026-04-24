//! Per-bigram miss rate and timing — which *transitions* trip you
//! up.
//!
//! A bigram here is **two consecutive events in the same session**,
//! keyed on `(expected_first, expected_second)`. The second event's
//! correctness determines whether the bigram counts as a miss: the
//! first keystroke's correctness is already attributed to the
//! previous bigram (or to nothing, for the session's first
//! keystroke).
//!
//! Bigrams never pair across session boundaries — the gap between
//! one session's last event and the next session's first event has
//! no meaningful timing and no meaningful "transition." The view
//! groups events by session before walking pairs.
//!
//! **What this is good for:** finding transitions that are
//! physically awkward on your layout. A high miss rate on `th`
//! means "my hands stumble on the t→h motion specifically," which
//! is a data point you can't get from per-key stats alone (`t` and
//! `h` might each be 95% accurate independently and still have a
//! bad transition between them).
//!
//! **What this is *not* good for:** bigram *frequency* on natural
//! text — this counts only what the user actually typed, not what
//! a corpus says English produces. The corpus side of "frequent
//! *and* expensive *and* personally weak" lives in drift; pairing
//! the two is a cross-reference view that combines both signals.

use std::collections::HashMap;

use anyhow::Result;

use crate::event::Event;
use crate::session::SessionId;
use crate::{EventFilter, EventStore};

/// Aggregate counts + timing for one bigram transition.
#[derive(Debug, Clone, Copy, Default)]
pub struct BigramStats {
    /// Total times this `(first, second)` pair appeared as
    /// consecutive events.
    pub count: u64,
    /// Occurrences where the second keystroke was a miss.
    pub miss_count: u64,
    /// Sum of `delta_ms` between first and second keystroke across
    /// all occurrences. Some pairs may have contributed `None` if
    /// the gap exceeded the idle threshold — those pairs count
    /// toward `count` and `miss_count` but not toward timing.
    pub total_delta_ms: u64,
    /// How many occurrences contributed to `total_delta_ms`. May
    /// be less than `count` if some pairs had `None` deltas.
    pub timed_count: u64,
}

impl BigramStats {
    /// Miss rate as 0.0..=1.0. Returns 0.0 on empty counts.
    pub fn miss_rate(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.miss_count as f64 / self.count as f64
    }

    /// Average inter-keystroke delay in milliseconds, or `None` if
    /// no occurrences contributed timing.
    pub fn avg_delta_ms(&self) -> Option<f64> {
        if self.timed_count == 0 {
            return None;
        }
        Some(self.total_delta_ms as f64 / self.timed_count as f64)
    }
}

/// Compute per-bigram stats over events matching `filter`. Pairs
/// are formed *within* each session — no cross-session pairing.
///
/// Character keys are lowercased to match the heat view's
/// convention (so `'T'→'H'` and `'t'→'h'` accumulate into the
/// same bucket).
pub fn bigram_stats(
    store: &dyn EventStore,
    filter: &EventFilter,
) -> Result<HashMap<(char, char), BigramStats>> {
    // Events arrive in chronological order across all matching
    // sessions. Group by session_id so we pair within each, not
    // across boundaries.
    let mut by_session: HashMap<SessionId, Vec<Event>> = HashMap::new();
    for event in store.events(filter)? {
        let event = event?;
        by_session.entry(event.session_id).or_default().push(event);
    }

    let mut out: HashMap<(char, char), BigramStats> = HashMap::new();
    for events in by_session.values() {
        for window in events.windows(2) {
            let [first, second] = window else { continue };
            let key = (
                first.expected.to_ascii_lowercase(),
                second.expected.to_ascii_lowercase(),
            );
            let stats = out.entry(key).or_default();
            stats.count += 1;
            if !second.correct {
                stats.miss_count += 1;
            }
            if let Some(ms) = second.delta_ms {
                stats.total_delta_ms += ms as u64;
                stats.timed_count += 1;
            }
        }
    }
    Ok(out)
}

/// Convenience: bigrams sorted by worst miss rate first, limited
/// to pairs with at least `min_count` occurrences so one-offs
/// don't dominate the ranking.
///
/// Returns `Vec<((char, char), BigramStats)>` sorted descending by
/// miss rate, ties broken by descending count.
pub fn worst_bigrams(
    store: &dyn EventStore,
    filter: &EventFilter,
    min_count: u64,
) -> Result<Vec<((char, char), BigramStats)>> {
    let mut pairs: Vec<_> = bigram_stats(store, filter)?
        .into_iter()
        .filter(|(_, s)| s.count >= min_count)
        .collect();
    pairs.sort_by(|a, b| {
        b.1.miss_rate()
            .partial_cmp(&a.1.miss_rate())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.1.count.cmp(&a.1.count))
    });
    Ok(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miss_rate_and_avg_empty() {
        let s = BigramStats::default();
        assert_eq!(s.miss_rate(), 0.0);
        assert_eq!(s.avg_delta_ms(), None);
    }

    #[test]
    fn miss_rate_basic() {
        let s = BigramStats {
            count: 10,
            miss_count: 3,
            total_delta_ms: 1000,
            timed_count: 10,
        };
        assert_eq!(s.miss_rate(), 0.3);
        assert_eq!(s.avg_delta_ms(), Some(100.0));
    }

    /// Timing skipped when no pair contributed a `delta_ms`.
    #[test]
    fn avg_delta_ignores_untimed() {
        let s = BigramStats {
            count: 5,
            miss_count: 0,
            total_delta_ms: 0,
            timed_count: 0,
        };
        assert_eq!(s.avg_delta_ms(), None);
    }
}
