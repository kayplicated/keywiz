//! Rhythm view — derived shape-of-typing numbers beyond WPM/accuracy.
//!
//! WPM tells you average speed; this view tells you the *shape* of
//! that speed. A 60-WPM user with 40 ms/200 ms alternating rhythm
//! has very different ergonomics from a steady 60-WPM user, even
//! though the headline number agrees.
//!
//! All helpers here take a pre-collected `&[Event]` rather than the
//! store directly. Callers fetch once via [`collect_events`] and
//! reuse the slice across every rhythm call — the P1 overview page
//! reads ~six rhythm numbers and it's wasteful to stream the events
//! six times.

use std::collections::HashMap;

use anyhow::Result;

use crate::event::Event;
use crate::session::SessionId;
use crate::{EventFilter, EventStore};

/// Materialize the events matching a filter into a Vec. One query,
/// many rhythm readers.
pub fn collect_events(store: &dyn EventStore, filter: &EventFilter) -> Result<Vec<Event>> {
    let mut events = Vec::new();
    for event in store.events(filter)? {
        events.push(event?);
    }
    Ok(events)
}

/// Median inter-keystroke delay in milliseconds across timed events.
/// Returns `None` when no event contributed timing (first-of-session
/// and idle-gap events don't count).
pub fn median_delta_ms(events: &[Event]) -> Option<f64> {
    let mut deltas: Vec<u32> = events.iter().filter_map(|e| e.delta_ms).collect();
    if deltas.is_empty() {
        return None;
    }
    deltas.sort_unstable();
    let mid = deltas.len() / 2;
    Some(if deltas.len().is_multiple_of(2) {
        (deltas[mid - 1] as f64 + deltas[mid] as f64) / 2.0
    } else {
        deltas[mid] as f64
    })
}

/// 95th percentile inter-keystroke delay — where you stall. Returns
/// `None` with no timed events.
pub fn p95_delta_ms(events: &[Event]) -> Option<f64> {
    let mut deltas: Vec<u32> = events.iter().filter_map(|e| e.delta_ms).collect();
    if deltas.is_empty() {
        return None;
    }
    deltas.sort_unstable();
    // `((n - 1) * 0.95).round()` lands on an inclusive index so
    // n=1 picks index 0 and n=100 picks index 94. Good enough for
    // display; we're not doing statistics.
    let idx = (((deltas.len() - 1) as f64) * 0.95).round() as usize;
    Some(deltas[idx] as f64)
}

/// Standard deviation of inter-keystroke delays in ms. Returns
/// `None` when fewer than two timed events exist (no meaningful
/// deviation).
pub fn stddev_delta_ms(events: &[Event]) -> Option<f64> {
    let deltas: Vec<u32> = events.iter().filter_map(|e| e.delta_ms).collect();
    if deltas.len() < 2 {
        return None;
    }
    let n = deltas.len() as f64;
    let mean = deltas.iter().map(|&d| d as f64).sum::<f64>() / n;
    let var = deltas
        .iter()
        .map(|&d| {
            let diff = d as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / n;
    Some(var.sqrt())
}

/// Consistency score — coefficient of variation (stddev / mean) of
/// inter-keystroke delays, as a **percentage**. Lower is steadier.
/// Typical human values: ~25% very consistent, ~50% mixed,
/// ~80%+ erratic. `None` if fewer than two timed events.
pub fn consistency_pct(events: &[Event]) -> Option<f64> {
    let deltas: Vec<u32> = events.iter().filter_map(|e| e.delta_ms).collect();
    if deltas.len() < 2 {
        return None;
    }
    let n = deltas.len() as f64;
    let mean = deltas.iter().map(|&d| d as f64).sum::<f64>() / n;
    if mean == 0.0 {
        return None;
    }
    let var = deltas
        .iter()
        .map(|&d| {
            let diff = d as f64 - mean;
            diff * diff
        })
        .sum::<f64>()
        / n;
    Some((var.sqrt() / mean) * 100.0)
}

/// Fastest inter-keystroke delay in ms — your peak hand speed on a
/// single transition. `None` with no timed events.
pub fn fastest_delta_ms(events: &[Event]) -> Option<u32> {
    events.iter().filter_map(|e| e.delta_ms).min()
}

/// Slowest timed inter-keystroke delay in ms. Idle gaps over the
/// crate's threshold collapse to `None` at record time so this
/// stops short of AFK pauses.
pub fn slowest_delta_ms(events: &[Event]) -> Option<u32> {
    events.iter().filter_map(|e| e.delta_ms).max()
}

/// Longest run of consecutive correct keystrokes in a single
/// session. Streaks never cross session boundaries.
pub fn longest_correct_streak(events: &[Event]) -> u32 {
    let mut by_session: HashMap<SessionId, u32> = HashMap::new();
    let mut best_overall = 0u32;
    let mut current_run: HashMap<SessionId, u32> = HashMap::new();
    for event in events {
        let run = current_run.entry(event.session_id).or_default();
        if event.correct {
            *run += 1;
            let best = by_session.entry(event.session_id).or_default();
            if *run > *best {
                *best = *run;
            }
            if *run > best_overall {
                best_overall = *run;
            }
        } else {
            *run = 0;
        }
    }
    best_overall
}

/// Peak rolling WPM over any `window_ms` contiguous window within
/// a single session. The "burst" ceiling — what the user can do at
/// their best, vs the session average WPM.
///
/// Walks each session's events with two pointers: advance the right
/// edge, collapse the left edge while the window is too wide,
/// compute net WPM within the window, track the max. No allocation
/// beyond the per-session grouping.
pub fn burst_wpm(events: &[Event], window_ms: u32) -> f64 {
    const CHARS_PER_WORD: f64 = 5.0;
    const MS_PER_MINUTE: f64 = 60_000.0;
    let window = window_ms as i64;
    if window <= 0 {
        return 0.0;
    }

    let mut by_session: HashMap<SessionId, Vec<&Event>> = HashMap::new();
    for event in events {
        by_session.entry(event.session_id).or_default().push(event);
    }

    let mut peak = 0.0f64;
    for session_events in by_session.values() {
        let events = session_events.as_slice();
        let mut left = 0usize;
        let mut correct_in_window = 0u64;
        for right in 0..events.len() {
            if events[right].correct {
                correct_in_window += 1;
            }
            while left < right
                && events[right].ts_ms.saturating_sub(events[left].ts_ms) > window
            {
                if events[left].correct {
                    correct_in_window = correct_in_window.saturating_sub(1);
                }
                left += 1;
            }
            // Need at least ~one window's worth of span before the
            // number is meaningful; small spans blow up the rate.
            let span_ms = events[right].ts_ms.saturating_sub(events[left].ts_ms);
            if span_ms < window / 2 {
                continue;
            }
            let minutes = span_ms as f64 / MS_PER_MINUTE;
            if minutes <= 0.0 {
                continue;
            }
            let wpm = (correct_in_window as f64 / CHARS_PER_WORD) / minutes;
            if wpm > peak {
                peak = wpm;
            }
        }
    }
    peak
}

/// Net WPM across a contiguous slice of events. Sums active_ms
/// (delta_ms for timed events) and counts correct keystrokes
/// within the slice; returns `None` if the slice contributed no
/// timed events (so the WPM would be undefined).
fn wpm_over_slice(slice: &[Event]) -> Option<f64> {
    const CHARS_PER_WORD: f64 = 5.0;
    const MS_PER_MINUTE: f64 = 60_000.0;
    let mut active_ms: u64 = 0;
    let mut correct: u64 = 0;
    for ev in slice {
        if ev.correct {
            correct += 1;
        }
        if let Some(ms) = ev.delta_ms {
            active_ms += ms as u64;
        }
    }
    if active_ms == 0 {
        return None;
    }
    let minutes = active_ms as f64 / MS_PER_MINUTE;
    Some((correct as f64 / CHARS_PER_WORD) / minutes)
}

/// Net WPM over the first `window` events of `events` — the
/// "warmup" segment. Returns `None` when the slice is shorter
/// than `3 * window` keystrokes, on the reasoning that a session
/// has to be long enough for warmup, steady, and end segments
/// to all be meaningful and non-overlapping before the split
/// carries signal.
pub fn warmup_wpm(events: &[Event], window: usize) -> Option<f64> {
    if window == 0 || events.len() < window * 3 {
        return None;
    }
    wpm_over_slice(&events[..window])
}

/// Net WPM over the last `window` events — the "end" or cooldown
/// segment. Same minimum-length guard as [`warmup_wpm`].
pub fn end_wpm(events: &[Event], window: usize) -> Option<f64> {
    if window == 0 || events.len() < window * 3 {
        return None;
    }
    let start = events.len() - window;
    wpm_over_slice(&events[start..])
}

/// Net WPM over the middle events — everything between the
/// warmup window and the end window. Returns the "steady state"
/// WPM the user settles into. Same minimum-length guard as
/// [`warmup_wpm`].
pub fn steady_wpm(events: &[Event], window: usize) -> Option<f64> {
    if window == 0 || events.len() < window * 3 {
        return None;
    }
    let end_start = events.len() - window;
    wpm_over_slice(&events[window..end_start])
}

/// Bucket APM values across the session timeline for sparkline
/// rendering. Returns a `Vec<f64>` of APM per bucket in chrono
/// order. A bucket with no timed events reads as 0.
///
/// `bucket_count` is the desired resolution; we slice the span
/// from first to last event into that many equal buckets.
pub fn apm_buckets(events: &[Event], bucket_count: usize) -> Vec<f64> {
    const MS_PER_MINUTE: f64 = 60_000.0;
    if events.is_empty() || bucket_count == 0 {
        return Vec::new();
    }
    let first_ts = events.iter().map(|e| e.ts_ms).min().unwrap_or(0);
    let last_ts = events.iter().map(|e| e.ts_ms).max().unwrap_or(0);
    let span = (last_ts - first_ts).max(1);
    let bucket_span = (span as f64 / bucket_count as f64).max(1.0);

    let mut counts = vec![0u64; bucket_count];
    for event in events {
        let offset = (event.ts_ms - first_ts) as f64;
        let idx = (offset / bucket_span).floor() as usize;
        let idx = idx.min(bucket_count - 1);
        counts[idx] += 1;
    }
    let bucket_minutes = bucket_span / MS_PER_MINUTE;
    counts
        .into_iter()
        .map(|c| if bucket_minutes > 0.0 { c as f64 / bucket_minutes } else { 0.0 })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(session: i64, ts: i64, expected: char, typed: char, delta: Option<u32>) -> Event {
        Event {
            session_id: SessionId(session),
            ts_ms: ts,
            expected,
            typed,
            correct: expected == typed,
            delta_ms: delta,
        }
    }

    #[test]
    fn empty_rhythm_returns_none() {
        let events: Vec<Event> = vec![];
        assert_eq!(median_delta_ms(&events), None);
        assert_eq!(p95_delta_ms(&events), None);
        assert_eq!(stddev_delta_ms(&events), None);
        assert_eq!(consistency_pct(&events), None);
        assert_eq!(fastest_delta_ms(&events), None);
        assert_eq!(slowest_delta_ms(&events), None);
        assert_eq!(longest_correct_streak(&events), 0);
        assert_eq!(burst_wpm(&events, 5000), 0.0);
    }

    #[test]
    fn median_odd_and_even() {
        let odd = vec![
            ev(1, 0, 'a', 'a', Some(100)),
            ev(1, 0, 'a', 'a', Some(200)),
            ev(1, 0, 'a', 'a', Some(300)),
        ];
        assert_eq!(median_delta_ms(&odd), Some(200.0));
        let even = vec![
            ev(1, 0, 'a', 'a', Some(100)),
            ev(1, 0, 'a', 'a', Some(200)),
            ev(1, 0, 'a', 'a', Some(300)),
            ev(1, 0, 'a', 'a', Some(400)),
        ];
        assert_eq!(median_delta_ms(&even), Some(250.0));
    }

    #[test]
    fn p95_picks_correct_index() {
        // 100 deltas 1..=100 means the sorted array is [1, 2, ..., 100]
        // at indices 0..=99. The 95th percentile by our definition
        // (`round((n-1) * 0.95)`) is index 94, which holds value 95.
        let mut events = Vec::new();
        for i in 1..=100u32 {
            events.push(ev(1, 0, 'a', 'a', Some(i)));
        }
        assert_eq!(p95_delta_ms(&events), Some(95.0));
    }

    #[test]
    fn p95_single_sample_is_that_sample() {
        // Single-element array: index 0, value 42.
        let events = vec![ev(1, 0, 'a', 'a', Some(42))];
        assert_eq!(p95_delta_ms(&events), Some(42.0));
    }

    #[test]
    fn consistency_zero_for_constant_rhythm() {
        let events = vec![
            ev(1, 0, 'a', 'a', Some(200)),
            ev(1, 0, 'a', 'a', Some(200)),
            ev(1, 0, 'a', 'a', Some(200)),
        ];
        assert_eq!(consistency_pct(&events), Some(0.0));
    }

    #[test]
    fn streak_resets_on_miss_and_across_sessions() {
        let events = vec![
            ev(1, 0, 'a', 'a', None),
            ev(1, 0, 'a', 'a', Some(100)),
            ev(1, 0, 'a', 'a', Some(100)),
            ev(1, 0, 'a', 'b', Some(100)),
            ev(1, 0, 'a', 'a', Some(100)),
            ev(1, 0, 'a', 'a', Some(100)),
            // New session — correct count restarts independent
            ev(2, 0, 'a', 'a', None),
            ev(2, 0, 'a', 'a', Some(100)),
        ];
        assert_eq!(longest_correct_streak(&events), 3);
    }

    #[test]
    fn burst_wpm_uniform_typing_exact() {
        // 10 correct keystrokes at ts=0,100,..,900 with window=1000.
        // The algorithm emits a candidate WPM for every `right`
        // position whose span back to `left` is ≥ window/2 = 500.
        // The peak over those candidates is at right=5: span = 500
        // ms, 6 correct chars (indices 0..=5) → net WPM =
        // (6/5) / (500/60_000) = 144. This is deliberate — a
        // "burst" is the fastest rate seen in any ≥ half-window
        // span, not just the full-window reading.
        let mut events = Vec::new();
        for i in 0..10 {
            events.push(ev(1, i * 100, 'a', 'a', Some(100)));
        }
        let peak = burst_wpm(&events, 1000);
        let expected = (6.0 / 5.0) / (500.0 / 60_000.0);
        assert!(
            (peak - expected).abs() < 1e-6,
            "expected {expected:.4} WPM burst, got {peak:.4}"
        );
    }

    #[test]
    fn burst_wpm_ignores_misses_in_numerator() {
        // Same timeline as `burst_wpm_uniform_typing_exact` but
        // every odd-indexed keystroke is wrong. The algorithm
        // evaluates every right position with span ≥ 500 ms and
        // keeps the peak. Candidate WPMs:
        //   right=5 (span 500, 3 correct): 72.0
        //   right=6 (span 600, 4 correct): 80.0 ← peak
        //   right=7 (span 700, 4 correct): 68.57
        //   right=8 (span 800, 5 correct): 75.0
        //   right=9 (span 900, 5 correct): 66.67
        // Peak at right=6 = (4/5) / (600/60_000) = 80.0. Asserts
        // that misses correctly *never* enter the numerator.
        let mut events = Vec::new();
        for i in 0..10u32 {
            let typed = if i.is_multiple_of(2) { 'a' } else { 'x' };
            events.push(ev(1, (i * 100) as i64, 'a', typed, Some(100)));
        }
        let peak = burst_wpm(&events, 1000);
        let expected = (4.0 / 5.0) / (600.0 / 60_000.0);
        assert!(
            (peak - expected).abs() < 1e-6,
            "expected {expected:.4} WPM burst, got {peak:.4}"
        );
    }

    #[test]
    fn apm_buckets_distribute_events() {
        // 4 events across a 1500 ms span with 2 buckets. Span = 1500,
        // bucket_span = 750 ms. Event offsets: 0, 500, 999, 1500.
        // Bucket 0 covers [0, 750): events at 0 and 500 → 2 events.
        // Bucket 1 covers [750, 1500]: events at 999 and 1500 → 2
        // events (1500 clamps to the last bucket).
        // APM per bucket = count / (bucket_span / 60_000 ms/min)
        // = 2 / 0.0125 = 160.0.
        let events = vec![
            ev(1, 0, 'a', 'a', None),
            ev(1, 500, 'a', 'a', Some(500)),
            ev(1, 999, 'a', 'a', Some(499)),
            ev(1, 1500, 'a', 'a', Some(501)),
        ];
        let buckets = apm_buckets(&events, 2);
        assert_eq!(buckets.len(), 2);
        let expected = 2.0 / (750.0 / 60_000.0);
        assert!(
            (buckets[0] - expected).abs() < 1e-6,
            "bucket[0] expected {expected}, got {}",
            buckets[0]
        );
        assert!(
            (buckets[1] - expected).abs() < 1e-6,
            "bucket[1] expected {expected}, got {}",
            buckets[1]
        );
    }

    #[test]
    fn session_arc_returns_none_when_too_short() {
        // Window=30 requires at least 90 events. 89 events → all
        // three segments return None.
        let mut events = Vec::new();
        for i in 0..89 {
            events.push(ev(1, i * 100, 'a', 'a', Some(100)));
        }
        assert_eq!(warmup_wpm(&events, 30), None);
        assert_eq!(steady_wpm(&events, 30), None);
        assert_eq!(end_wpm(&events, 30), None);
    }

    #[test]
    fn session_arc_boundary_at_exactly_3n() {
        // 90 events with window=30 is the smallest length that
        // satisfies the guard. All three segments must return Some.
        let mut events = Vec::new();
        // First event has no delta (the session's first keystroke).
        events.push(ev(1, 0, 'a', 'a', None));
        for i in 1..90i64 {
            events.push(ev(1, i * 100, 'a', 'a', Some(100)));
        }
        // Warmup: events [0..30). Active_ms = 29 * 100 = 2900 (the
        // first event contributes no delta). 30 correct / 5 = 6
        // words in 2900 ms = 2.9/60 minutes. WPM = 6 / (2.9/60) ≈
        // 124.138.
        let warmup = warmup_wpm(&events, 30).unwrap();
        let expected_warmup = (30.0 / 5.0) / (2900.0 / 60_000.0);
        assert!(
            (warmup - expected_warmup).abs() < 1e-6,
            "warmup {warmup:.4} vs expected {expected_warmup:.4}"
        );
        // Steady: events [30..60). 30 events, all timed (every one
        // carries a delta of 100). Active_ms = 3000. WPM = 6 / 0.05
        // = 120.
        let steady = steady_wpm(&events, 30).unwrap();
        let expected_steady = (30.0 / 5.0) / (3000.0 / 60_000.0);
        assert!(
            (steady - expected_steady).abs() < 1e-6,
            "steady {steady:.4} vs expected {expected_steady:.4}"
        );
        // End: events [60..90). Same shape as steady.
        let end = end_wpm(&events, 30).unwrap();
        let expected_end = (30.0 / 5.0) / (3000.0 / 60_000.0);
        assert!((end - expected_end).abs() < 1e-6);
    }

    #[test]
    fn session_arc_speeding_up_shows_warmup_below_end() {
        // Warmup events are spaced 200 ms apart (slow); end events
        // are spaced 50 ms apart (fast). Steady in the middle at
        // 100 ms. Expect warmup < steady < end.
        let mut events = Vec::new();
        events.push(ev(1, 0, 'a', 'a', None));
        // 30 warmup events at 200 ms each.
        let mut ts = 0i64;
        for _ in 1..30 {
            ts += 200;
            events.push(ev(1, ts, 'a', 'a', Some(200)));
        }
        // 30 steady events at 100 ms each.
        for _ in 30..60 {
            ts += 100;
            events.push(ev(1, ts, 'a', 'a', Some(100)));
        }
        // 30 end events at 50 ms each.
        for _ in 60..90 {
            ts += 50;
            events.push(ev(1, ts, 'a', 'a', Some(50)));
        }
        let w = warmup_wpm(&events, 30).unwrap();
        let s = steady_wpm(&events, 30).unwrap();
        let e = end_wpm(&events, 30).unwrap();
        assert!(
            w < s && s < e,
            "expected warmup < steady < end, got {w:.2} / {s:.2} / {e:.2}"
        );
    }

    #[test]
    fn session_arc_slowing_down_shows_warmup_above_end() {
        // Inverse of the speed-up test — classic fatigue curve.
        let mut events = Vec::new();
        events.push(ev(1, 0, 'a', 'a', None));
        let mut ts = 0i64;
        for _ in 1..30 {
            ts += 50;
            events.push(ev(1, ts, 'a', 'a', Some(50)));
        }
        for _ in 30..60 {
            ts += 100;
            events.push(ev(1, ts, 'a', 'a', Some(100)));
        }
        for _ in 60..90 {
            ts += 200;
            events.push(ev(1, ts, 'a', 'a', Some(200)));
        }
        let w = warmup_wpm(&events, 30).unwrap();
        let s = steady_wpm(&events, 30).unwrap();
        let e = end_wpm(&events, 30).unwrap();
        assert!(
            w > s && s > e,
            "expected warmup > steady > end, got {w:.2} / {s:.2} / {e:.2}"
        );
    }

    #[test]
    fn session_arc_misses_drop_numerator() {
        // 90 events at 100 ms each, but the last 30 have a 50% miss
        // rate. End WPM should be exactly half the steady WPM
        // because half the correct-count in the same time window.
        let mut events = Vec::new();
        events.push(ev(1, 0, 'a', 'a', None));
        for i in 1i64..60 {
            events.push(ev(1, i * 100, 'a', 'a', Some(100)));
        }
        for i in 60i64..90 {
            // Alternate correct / wrong so every other event is a
            // miss. 60..90 is 30 events, split 15 / 15.
            let typed = if (i % 2) == 0 { 'a' } else { 'x' };
            events.push(ev(1, i * 100, 'a', typed, Some(100)));
        }
        let steady = steady_wpm(&events, 30).unwrap();
        let end = end_wpm(&events, 30).unwrap();
        // Steady has 30/30 correct in 3000 ms. End has 15/30 correct
        // in 3000 ms. Ratio = 0.5.
        let ratio = end / steady;
        assert!(
            (ratio - 0.5).abs() < 1e-6,
            "expected end/steady = 0.5, got {ratio:.4}"
        );
    }

    #[test]
    fn apm_buckets_empty_bucket_reads_zero() {
        // Events at ts 0, 100, 200 with 5 buckets. Span = 200,
        // bucket_span = 40 ms. Event-to-bucket mapping
        // (floor(offset / 40)):
        //   ts=0   → bucket 0
        //   ts=100 → bucket 2 (100/40 = 2.5, floor 2)
        //   ts=200 → bucket 4 (clamped from floor 5)
        // So buckets 1 and 3 are empty and MUST read as 0.0 —
        // not "small" or "epsilon," exactly zero.
        let events = vec![
            ev(1, 0, 'a', 'a', None),
            ev(1, 100, 'a', 'a', Some(100)),
            ev(1, 200, 'a', 'a', Some(100)),
        ];
        let buckets = apm_buckets(&events, 5);
        assert_eq!(buckets.len(), 5);
        assert_eq!(buckets[1], 0.0, "bucket 1 should be empty");
        assert_eq!(buckets[3], 0.0, "bucket 3 should be empty");
        // Sanity: the non-empty ones are > 0.
        assert!(buckets[0] > 0.0);
        assert!(buckets[2] > 0.0);
        assert!(buckets[4] > 0.0);
    }
}
