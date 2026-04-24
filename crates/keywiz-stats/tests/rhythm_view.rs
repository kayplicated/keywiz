//! Integration tests for `views::rhythm` that go through the store.
//!
//! The view file's unit tests operate on pre-built `Vec<Event>`
//! slices. These tests round-trip through `Stats::record` and
//! `collect_events`, covering the gap where timing data from the
//! facade reaches the rhythm helpers.

mod common;

use keywiz_stats::store::memory::MemoryStore;
use keywiz_stats::views::rhythm;
use keywiz_stats::{EventFilter, EventStore, Stats};

use common::fixtures;

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
fn collect_events_returns_recorded_order() {
    let stats = recorded_at(&[
        (0, 'a', 'a'),
        (100, 'b', 'b'),
        (200, 'c', 'c'),
    ]);
    let events = rhythm::collect_events(stats.store(), &EventFilter::default()).unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].expected, 'a');
    assert_eq!(events[2].expected, 'c');
    // First event has no delta; subsequent deltas are ms between
    // consecutive recorded timestamps.
    assert_eq!(events[0].delta_ms, None);
    assert_eq!(events[1].delta_ms, Some(100));
    assert_eq!(events[2].delta_ms, Some(100));
}

#[test]
fn median_from_stored_events_matches_math() {
    // ts spacing 100, 150, 200 → deltas None, 100, 150, 200.
    // Timed deltas sorted: [100, 150, 200], median = 150.
    let stats = recorded_at(&[
        (0, 'a', 'a'),
        (100, 'b', 'b'),
        (250, 'c', 'c'),
        (450, 'd', 'd'),
    ]);
    let events = rhythm::collect_events(stats.store(), &EventFilter::default()).unwrap();
    assert_eq!(rhythm::median_delta_ms(&events), Some(150.0));
}

#[test]
fn streak_counts_across_stored_events() {
    // 2 correct, 1 miss, 3 correct → longest run = 3.
    let stats = recorded_at(&[
        (0, 'a', 'a'),
        (100, 'b', 'b'),
        (200, 'c', 'x'),
        (300, 'd', 'd'),
        (400, 'e', 'e'),
        (500, 'f', 'f'),
    ]);
    let events = rhythm::collect_events(stats.store(), &EventFilter::default()).unwrap();
    assert_eq!(rhythm::longest_correct_streak(&events), 3);
}

#[test]
fn idle_gap_collapses_delta_to_none() {
    // Gap of 15 seconds (> 10s idle threshold) drops the delta at
    // record time. Median should only consider the first gap.
    let stats = recorded_at(&[
        (0, 'a', 'a'),
        (100, 'b', 'b'),
        // 15s later — idle skip, delta_ms stored as None.
        (15_100, 'c', 'c'),
    ]);
    let events = rhythm::collect_events(stats.store(), &EventFilter::default()).unwrap();
    assert_eq!(events[1].delta_ms, Some(100));
    assert_eq!(events[2].delta_ms, None);
    // Only one timed delta → median equals that delta.
    assert_eq!(rhythm::median_delta_ms(&events), Some(100.0));
}
