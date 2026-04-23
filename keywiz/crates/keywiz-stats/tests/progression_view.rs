//! Integration tests for `views::progression` against a live store.
//!
//! Unit tests in the view file exercise `BucketStats`'s derivations.
//! These tests drive `bucket_stats` end-to-end: record events at
//! known timestamps, ask for bucket rollups at specified ranges,
//! assert that events land in the intended bucket and that the
//! per-bucket math matches what the event stream says.

mod common;

use keywiz_stats::store::memory::MemoryStore;
use keywiz_stats::views::progression::bucket_stats;
use keywiz_stats::{EventFilter, EventStore, Stats};

use common::fixtures;

/// Fresh `Stats` with one session; record the given
/// `(ts_ms, expected, typed)` tuples in order.
fn recorded_at(seq: &[(i64, char, char)]) -> Stats {
    let store = Box::new(MemoryStore::new()) as Box<dyn EventStore>;
    let mut stats = Stats::new(store);
    let l = fixtures::layout("l", &"a".repeat(64));
    let k = fixtures::keyboard("k", &"b".repeat(64));
    stats.begin_session(&l, &k, "drill", None, 0).unwrap();
    for (ts, expected, typed) in seq {
        stats.record(*expected, *typed, *ts).unwrap();
    }
    stats
}

#[test]
fn events_land_in_their_timestamp_bucket() {
    // Three events at ts 50, 150, 250. Ranges: [0, 100), [100, 200),
    // [200, 300). Each bucket should see exactly one event.
    let stats = recorded_at(&[(50, 'a', 'a'), (150, 'b', 'b'), (250, 'c', 'c')]);
    let ranges = vec![(0i64, 100i64), (100, 200), (200, 300)];
    let buckets = bucket_stats(stats.store(), &EventFilter::default(), &ranges).unwrap();
    assert_eq!(buckets.len(), 3);
    assert_eq!(buckets[0].total_events, 1);
    assert_eq!(buckets[1].total_events, 1);
    assert_eq!(buckets[2].total_events, 1);
    assert_eq!(buckets[0].from_ms, 0);
    assert_eq!(buckets[0].until_ms, 100);
}

#[test]
fn empty_bucket_has_zero_counts() {
    // One event at ts=50, but we ask for three ranges: events in
    // the second, nothing in 1st or 3rd.
    let stats = recorded_at(&[(150, 'a', 'a')]);
    let ranges = vec![(0i64, 100i64), (100, 200), (200, 300)];
    let buckets = bucket_stats(stats.store(), &EventFilter::default(), &ranges).unwrap();
    assert_eq!(buckets.len(), 3);
    assert_eq!(buckets[0].total_events, 0);
    assert!(buckets[0].is_empty());
    assert_eq!(buckets[1].total_events, 1);
    assert_eq!(buckets[2].total_events, 0);
}

#[test]
fn correct_events_count_towards_correct_only() {
    // Four events in one bucket: 3 correct, 1 miss.
    let stats = recorded_at(&[
        (10, 'a', 'a'),
        (20, 'b', 'x'),
        (30, 'c', 'c'),
        (40, 'd', 'd'),
    ]);
    let ranges = vec![(0i64, 100i64)];
    let buckets = bucket_stats(stats.store(), &EventFilter::default(), &ranges).unwrap();
    assert_eq!(buckets.len(), 1);
    assert_eq!(buckets[0].total_events, 4);
    assert_eq!(buckets[0].total_correct, 3);
    assert_eq!(buckets[0].accuracy_pct(), 75.0);
}

#[test]
fn active_ms_is_sum_of_timed_deltas() {
    // First event has no delta (None). Events 2..4 each contribute
    // their delta_ms to active_ms. Spacing is 100 ms → active_ms = 300.
    let stats = recorded_at(&[
        (0, 'a', 'a'),
        (100, 'b', 'b'),
        (200, 'c', 'c'),
        (300, 'd', 'd'),
    ]);
    let ranges = vec![(0i64, 400i64)];
    let buckets = bucket_stats(stats.store(), &EventFilter::default(), &ranges).unwrap();
    assert_eq!(buckets[0].active_ms, 300);
    // 4 correct keystrokes in 0.005 minutes → APM =
    // 4 / 0.005 = 800; WPM = 800/5 = 160.
    assert!((buckets[0].apm() - 800.0).abs() < 1e-6);
    assert!((buckets[0].net_wpm() - 160.0).abs() < 1e-6);
}

#[test]
fn base_filter_narrows_results() {
    // Two sessions — we want only the first one's events to show.
    let store = Box::new(MemoryStore::new()) as Box<dyn EventStore>;
    let mut stats = Stats::new(store);
    let l = fixtures::layout("l", &"a".repeat(64));
    let k = fixtures::keyboard("k", &"b".repeat(64));

    let s1 = stats.begin_session(&l, &k, "drill", None, 0).unwrap();
    stats.record('a', 'a', 10).unwrap();
    stats.end_session(20).unwrap();

    stats.begin_session(&l, &k, "drill", None, 30).unwrap();
    stats.record('b', 'b', 40).unwrap();
    stats.record('c', 'c', 50).unwrap();

    let base = EventFilter {
        session_id: Some(s1),
        ..Default::default()
    };
    let ranges = vec![(0i64, 1000i64)];
    let buckets = bucket_stats(stats.store(), &base, &ranges).unwrap();
    assert_eq!(buckets[0].total_events, 1, "only session 1 counted");
}

#[test]
fn bucket_bounds_are_preserved() {
    // Whatever ranges the caller asked for come back in the output
    // — page labels format off these, so the round-trip matters.
    let stats = recorded_at(&[(50, 'a', 'a')]);
    let ranges = vec![(0i64, 100), (100, 200), (200, 300)];
    let buckets = bucket_stats(stats.store(), &EventFilter::default(), &ranges).unwrap();
    for (i, (from, until)) in ranges.iter().enumerate() {
        assert_eq!(buckets[i].from_ms, *from);
        assert_eq!(buckets[i].until_ms, *until);
    }
}
