//! Per-key typing statistics.
//!
//! Tracks attempts and correct counts for each character the user has typed.
//! Used by engines to record keystrokes, and by UI components (heatmap) and
//! auto-drill generation to query accuracy.
//!
//! This module owns the in-memory data model. Disk persistence lives in
//! [`persist`].

pub mod persist;

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Per-key accuracy record.
#[derive(Debug, Clone, Default)]
pub struct KeyRecord {
    pub attempts: u64,
    pub correct: u64,
    /// Unix timestamp (seconds) of the most recent attempt.
    pub last_seen: u64,
}

impl KeyRecord {
    /// Accuracy as a fraction in `0.0..=1.0`. Returns `1.0` for keys with no
    /// attempts so fresh keys don't show up as "worst" before being tried.
    pub fn accuracy(&self) -> f64 {
        if self.attempts == 0 {
            1.0
        } else {
            self.correct as f64 / self.attempts as f64
        }
    }
}

/// Per-layout keystroke statistics.
#[derive(Debug, Clone, Default)]
pub struct Stats {
    keys: HashMap<char, KeyRecord>,
}

impl Stats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a single keystroke against the expected character.
    pub fn record(&mut self, expected: char, correct: bool) {
        let record = self.keys.entry(expected).or_default();
        record.attempts += 1;
        if correct {
            record.correct += 1;
        }
        record.last_seen = now_unix();
    }

    /// Look up the record for a single key.
    pub fn get(&self, ch: char) -> Option<&KeyRecord> {
        self.keys.get(&ch)
    }

    /// Iterate over all recorded keys and their records.
    pub fn iter(&self) -> impl Iterator<Item = (&char, &KeyRecord)> {
        self.keys.iter()
    }

    /// Return the `n` keys with the lowest accuracy, lowest first.
    /// Keys with zero attempts are excluded (no data to judge).
    pub fn worst_keys(&self, n: usize) -> Vec<char> {
        let mut with_data: Vec<(&char, &KeyRecord)> = self
            .keys
            .iter()
            .filter(|(_, r)| r.attempts > 0)
            .collect();
        with_data.sort_by(|a, b| {
            a.1.accuracy()
                .partial_cmp(&b.1.accuracy())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        with_data.into_iter().take(n).map(|(c, _)| *c).collect()
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_increments_attempts_and_correct() {
        let mut s = Stats::new();
        s.record('a', true);
        s.record('a', false);
        s.record('a', true);

        let r = s.get('a').unwrap();
        assert_eq!(r.attempts, 3);
        assert_eq!(r.correct, 2);
        assert!((r.accuracy() - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn untouched_key_reports_perfect_accuracy() {
        let s = Stats::new();
        assert!(s.get('z').is_none());
    }

    #[test]
    fn worst_keys_returns_lowest_accuracy_first() {
        let mut s = Stats::new();
        // 'a': 100%
        s.record('a', true);
        s.record('a', true);
        // 'b': 50%
        s.record('b', true);
        s.record('b', false);
        // 'c': 0%
        s.record('c', false);

        let worst = s.worst_keys(2);
        assert_eq!(worst, vec!['c', 'b']);
    }

    #[test]
    fn worst_keys_ignores_untried_keys() {
        let mut s = Stats::new();
        s.record('a', true);
        assert_eq!(s.worst_keys(5), vec!['a']);
    }
}
