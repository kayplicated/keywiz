//! Per-key typing statistics.
//!
//! Tracks attempts, correct count, and accumulated heat for each character
//! the user has typed. Engines record keystrokes through this; the keyboard
//! heatmap reads from it to color keys, and the word selector reads from it
//! to bias practice toward struggling letters.
//!
//! This module owns the in-memory data model. Disk persistence lives in
//! [`persist`].

pub mod persist;
pub mod tracker;

pub use tracker::StatsTracker;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum heat a key can accumulate. Past this, extra wrong presses
/// don't push it further — the key is already "fully hot."
pub const MAX_HEAT: u32 = 20;

/// Correct presses needed to drop one heat step. Flat across all steps:
/// one miss = +1 step, two corrects = -1 step. Wrong presses don't wipe
/// accumulated cooling progress, so practice stays productive even when
/// you're still making mistakes.
pub const COOL_COST: u32 = 2;

/// Per-key record of accuracy and heat.
///
/// `heat` is the integer step (0..=MAX_HEAT) used by the keyboard
/// heatmap overlay: each wrong press bumps it up one step, each correct
/// press chips away at a cooling budget that — once filled — drops the
/// step by one.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeyRecord {
    pub attempts: u64,
    pub correct: u64,
    /// Unix timestamp (seconds) of the most recent attempt.
    pub last_seen: u64,
    /// Integer heat level, 0..=MAX_HEAT. Wrong presses add 1; correct
    /// presses reduce this after enough cooling has accumulated.
    #[serde(default)]
    pub heat: u32,
    /// Correct presses accumulated toward cooling this key one step.
    /// Resets to 0 when heat drops (or climbs from a wrong press).
    #[serde(default)]
    pub cooling_progress: u32,
}

impl KeyRecord {
    /// Apply the heat model for a single keystroke.
    /// - wrong press: heat += 1 (capped at MAX_HEAT); cooling progress is
    ///   preserved so practice stays productive through mistakes.
    /// - correct press on a hot key: add to cooling progress; every
    ///   `COOL_COST` correct presses drops heat by one step.
    /// - correct press on a cold key (heat = 0): nothing changes.
    fn update_heat(&mut self, correct: bool) {
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

/// Per-layout keystroke statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    keys: HashMap<char, KeyRecord>,
}

impl Stats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a single keystroke against the expected character.
    /// Uppercase letters are folded to lowercase so a missed capital still
    /// heats up the visible (lowercase) key on the keyboard widget.
    pub fn record(&mut self, expected: char, correct: bool) {
        let key = if expected.is_ascii_uppercase() {
            expected.to_ascii_lowercase()
        } else {
            expected
        };
        let record = self.keys.entry(key).or_default();
        record.attempts += 1;
        if correct {
            record.correct += 1;
        }
        record.last_seen = now_unix();
        record.update_heat(correct);
    }

    /// Look up the record for a single key.
    pub fn get(&self, ch: char) -> Option<&KeyRecord> {
        self.keys.get(&ch)
    }

    /// Iterate over all recorded keys and their records.
    pub fn iter(&self) -> impl Iterator<Item = (&char, &KeyRecord)> {
        self.keys.iter()
    }

    /// Total attempts summed across all keys.
    pub fn total_attempts(&self) -> u64 {
        self.keys.values().map(|r| r.attempts).sum()
    }

    /// Total correct presses summed across all keys.
    pub fn total_correct(&self) -> u64 {
        self.keys.values().map(|r| r.correct).sum()
    }

    /// Total incorrect presses summed across all keys.
    pub fn total_wrong(&self) -> u64 {
        self.total_attempts() - self.total_correct()
    }

    /// Aggregate accuracy as a percentage in `0.0..=100.0`.
    /// Returns `100.0` when no keys have been attempted.
    pub fn overall_accuracy(&self) -> f64 {
        let total = self.total_attempts();
        if total == 0 {
            100.0
        } else {
            (self.total_correct() as f64 / total as f64) * 100.0
        }
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
    }

    #[test]
    fn untouched_key_returns_none() {
        let s = Stats::new();
        assert!(s.get('z').is_none());
    }

    #[test]
    fn uppercase_miss_folds_into_lowercase_key() {
        let mut s = Stats::new();
        s.record('T', false);
        assert!(s.get('T').is_none());
        let r = s.get('t').unwrap();
        assert_eq!(r.attempts, 1);
        assert_eq!(r.correct, 0);
        assert_eq!(r.heat, 1);
    }

    #[test]
    fn heat_climbs_with_each_wrong_press() {
        let mut s = Stats::new();
        for expected in 1..=5 {
            s.record('a', false);
            assert_eq!(s.get('a').unwrap().heat, expected);
        }
    }

    #[test]
    fn heat_caps_at_max() {
        let mut s = Stats::new();
        for _ in 0..(MAX_HEAT + 10) {
            s.record('a', false);
        }
        assert_eq!(s.get('a').unwrap().heat, MAX_HEAT);
    }

    #[test]
    fn correct_presses_cool_a_hot_key_at_flat_cost() {
        let mut s = Stats::new();
        // Push heat to 3 — that's 3 steps to clear at 2 corrects each = 6.
        s.record('a', false);
        s.record('a', false);
        s.record('a', false);
        assert_eq!(s.get('a').unwrap().heat, 3);

        for expected_remaining in (0..3).rev() {
            s.record('a', true);
            assert_eq!(s.get('a').unwrap().heat, expected_remaining + 1);
            s.record('a', true);
            assert_eq!(s.get('a').unwrap().heat, expected_remaining);
        }
    }

    #[test]
    fn wrong_press_does_not_wipe_cooling_progress() {
        let mut s = Stats::new();
        s.record('a', false);
        s.record('a', false);
        // One correct press — not enough to cool yet.
        s.record('a', true);
        assert_eq!(s.get('a').unwrap().heat, 2);
        assert_eq!(s.get('a').unwrap().cooling_progress, 1);

        // Another wrong press bumps heat but keeps the cooling progress.
        s.record('a', false);
        assert_eq!(s.get('a').unwrap().heat, 3);
        assert_eq!(s.get('a').unwrap().cooling_progress, 1);

        // The next correct press finishes the cooling from that earlier +1.
        s.record('a', true);
        assert_eq!(s.get('a').unwrap().heat, 2);
        assert_eq!(s.get('a').unwrap().cooling_progress, 0);
    }

    #[test]
    fn correct_presses_on_a_cold_key_do_nothing() {
        let mut s = Stats::new();
        for _ in 0..100 {
            s.record('a', true);
        }
        let r = s.get('a').unwrap();
        assert_eq!(r.heat, 0);
        assert_eq!(r.cooling_progress, 0);
    }
}
