//! Per-key counts, miss rate, and timing — the "which individual
//! keys trip you up" view.
//!
//! Complements [`bigram`](crate::views::bigram): a single key can
//! be 98% accurate on its own and still pull its weight in a bad
//! transition (or vice versa). The stats page shows both so the
//! user can tell "this *key* is weak" from "this *transition* is
//! weak."
//!
//! Keys are folded to their lowercase form, matching [`heat`] and
//! [`bigram`] conventions. A missed capital heats the same key as
//! a missed lowercase.
//!
//! [`heat`]: crate::views::heat
//! [`bigram`]: crate::views::bigram

use std::collections::HashMap;

use anyhow::Result;

use crate::{EventFilter, EventStore};

/// Aggregate counts + timing for one character.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyStats {
    /// Times the user was asked to type this character.
    pub count: u64,
    /// Occurrences where the user's keystroke was wrong.
    pub miss_count: u64,
    /// Sum of `delta_ms` across timed occurrences. Events where
    /// `delta_ms` was `None` (idle gap, first-of-session) contribute
    /// to `count` but not to `total_delta_ms`.
    pub total_delta_ms: u64,
    /// How many occurrences contributed timing.
    pub timed_count: u64,
}

impl KeyStats {
    /// Miss rate as 0.0..=1.0. Returns 0.0 on empty counts.
    pub fn miss_rate(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.miss_count as f64 / self.count as f64
    }

    /// Average inter-keystroke delay in milliseconds, or `None`
    /// when no occurrences contributed timing.
    pub fn avg_delta_ms(&self) -> Option<f64> {
        if self.timed_count == 0 {
            return None;
        }
        Some(self.total_delta_ms as f64 / self.timed_count as f64)
    }
}

/// Per-character aggregates over events matching `filter`.
pub fn key_stats(
    store: &dyn EventStore,
    filter: &EventFilter,
) -> Result<HashMap<char, KeyStats>> {
    let mut out: HashMap<char, KeyStats> = HashMap::new();
    for event in store.events(filter)? {
        let event = event?;
        let key = event.expected.to_ascii_lowercase();
        let stats = out.entry(key).or_default();
        stats.count += 1;
        if !event.correct {
            stats.miss_count += 1;
        }
        if let Some(ms) = event.delta_ms {
            stats.total_delta_ms += ms as u64;
            stats.timed_count += 1;
        }
    }
    Ok(out)
}

/// Keys sorted worst-first by miss rate, limited to keys seen at
/// least `min_count` times so single-sample outliers don't dominate.
/// Ties broken by descending count — the more data we have, the
/// more confident the ranking.
pub fn worst_keys(
    store: &dyn EventStore,
    filter: &EventFilter,
    min_count: u64,
) -> Result<Vec<(char, KeyStats)>> {
    let mut keys: Vec<_> = key_stats(store, filter)?
        .into_iter()
        .filter(|(_, s)| s.count >= min_count)
        .collect();
    keys.sort_by(|a, b| {
        b.1.miss_rate()
            .partial_cmp(&a.1.miss_rate())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.1.count.cmp(&a.1.count))
    });
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_stats_zero_miss_rate() {
        let s = KeyStats::default();
        assert_eq!(s.miss_rate(), 0.0);
        assert_eq!(s.avg_delta_ms(), None);
    }

    #[test]
    fn miss_rate_basic() {
        let s = KeyStats {
            count: 10,
            miss_count: 3,
            total_delta_ms: 1200,
            timed_count: 10,
        };
        assert_eq!(s.miss_rate(), 0.3);
        assert_eq!(s.avg_delta_ms(), Some(120.0));
    }

    #[test]
    fn avg_delta_excludes_untimed_occurrences() {
        let s = KeyStats {
            count: 10,
            miss_count: 0,
            total_delta_ms: 300,
            timed_count: 3,
        };
        assert_eq!(s.avg_delta_ms(), Some(100.0));
    }
}
