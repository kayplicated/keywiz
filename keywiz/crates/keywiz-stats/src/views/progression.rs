//! Progression view — roll events up into time-range buckets.
//!
//! P2 answers "am I getting better?" by plotting headline numbers
//! across consecutive buckets at the user's chosen granularity
//! (day / week / month / year). This module turns a `Vec<(from_ms,
//! until_ms)>` bucket spec into `Vec<BucketStats>` — one row per
//! bucket with WPM / APM / accuracy / keystroke counts.
//!
//! Buckets are *ordered* as given — the caller owns the "last N
//! buckets ending at now" arithmetic; this view just computes what
//! sits inside each range.

use anyhow::Result;

use crate::views::wpm::SessionWpm;
use crate::{EventFilter, EventStore};

/// Stats for one contiguous time bucket.
#[derive(Debug, Clone, Copy)]
pub struct BucketStats {
    /// Bucket's `[from_ms, until_ms)` bounds — mirrors the spec the
    /// caller asked for, so labels can be formatted off these.
    pub from_ms: i64,
    pub until_ms: i64,
    /// Keystrokes recorded in the bucket.
    pub total_events: u64,
    /// Of those, correct.
    pub total_correct: u64,
    /// Active typing time in the bucket (sum of `delta_ms` over
    /// timed events). Skips the first-of-session keystroke and
    /// idle gaps.
    pub active_ms: u64,
}

impl BucketStats {
    /// Net WPM: correct_chars / 5 / minutes-of-active-time. Zero
    /// when the bucket has no active time.
    pub fn net_wpm(&self) -> f64 {
        if self.active_ms == 0 {
            return 0.0;
        }
        let minutes = self.active_ms as f64 / 60_000.0;
        (self.total_correct as f64 / 5.0) / minutes
    }

    /// Actions per minute of active time.
    pub fn apm(&self) -> f64 {
        if self.active_ms == 0 {
            return 0.0;
        }
        let minutes = self.active_ms as f64 / 60_000.0;
        self.total_events as f64 / minutes
    }

    /// Accuracy as a percentage 0..=100. Returns 100 on an empty
    /// bucket so an empty column in the table reads as "nothing
    /// wrong" rather than "zero accuracy."
    pub fn accuracy_pct(&self) -> f64 {
        if self.total_events == 0 {
            return 100.0;
        }
        (self.total_correct as f64 / self.total_events as f64) * 100.0
    }

    /// True when the bucket had no events — caller uses this to
    /// render a `—` instead of literal zeros.
    pub fn is_empty(&self) -> bool {
        self.total_events == 0
    }
}

/// Aggregate one bucket per `(from_ms, until_ms)` entry. Composes
/// the provided base filter (layout / keyboard / session scoping)
/// with the per-bucket time range. Returns buckets in the same
/// order as the input spec.
pub fn bucket_stats(
    store: &dyn EventStore,
    base: &EventFilter,
    ranges: &[(i64, i64)],
) -> Result<Vec<BucketStats>> {
    let mut out = Vec::with_capacity(ranges.len());
    for &(from, until) in ranges {
        let filter = EventFilter {
            from_ms: Some(from),
            until_ms: Some(until),
            ..base.clone()
        };
        let mut b = BucketStats {
            from_ms: from,
            until_ms: until,
            total_events: 0,
            total_correct: 0,
            active_ms: 0,
        };
        for ev in store.events(&filter)?.flatten() {
            b.total_events += 1;
            if ev.correct {
                b.total_correct += 1;
            }
            if let Some(ms) = ev.delta_ms {
                b.active_ms += ms as u64;
            }
        }
        out.push(b);
    }
    Ok(out)
}

/// Convenience: promote a bucket to a `SessionWpm` so callers can
/// reuse the existing WPM formatting helpers.
impl From<BucketStats> for SessionWpm {
    fn from(b: BucketStats) -> Self {
        SessionWpm {
            active_ms: b.active_ms,
            total_keystrokes: b.total_events,
            correct_keystrokes: b.total_correct,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bucket_reads_as_100_accuracy() {
        let b = BucketStats {
            from_ms: 0,
            until_ms: 1,
            total_events: 0,
            total_correct: 0,
            active_ms: 0,
        };
        assert_eq!(b.accuracy_pct(), 100.0);
        assert_eq!(b.net_wpm(), 0.0);
        assert_eq!(b.apm(), 0.0);
        assert!(b.is_empty());
    }

    #[test]
    fn canonical_minute_bucket() {
        // 50 correct / 50 total in one minute of active time →
        // 10 WPM, 50 APM, 100% accuracy.
        let b = BucketStats {
            from_ms: 0,
            until_ms: 60_000,
            total_events: 50,
            total_correct: 50,
            active_ms: 60_000,
        };
        assert_eq!(b.net_wpm(), 10.0);
        assert_eq!(b.apm(), 50.0);
        assert_eq!(b.accuracy_pct(), 100.0);
    }
}
