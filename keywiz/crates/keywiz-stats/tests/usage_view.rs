//! Integration tests for `views::usage`.
//!
//! Usage's logic is simple (count `expected` per char, log-normalize
//! against the max, drop anything below `MIN_PRESSES`). The goals
//! of these tests are to verify:
//!   - counts are correct,
//!   - MIN_PRESSES filters noise,
//!   - log normalization makes the max key saturate,
//!   - layout_hash filtering scopes correctly.

mod common;

use keywiz_stats::store::memory::MemoryStore;
use keywiz_stats::views::usage::{usage_map, usage_map_raw, MIN_PRESSES};
use keywiz_stats::{EventFilter, EventStore, Stats};

use common::fixtures;

/// Build a Stats populated with a sequence of `(expected, typed)`
/// pairs on a single layout/keyboard/session.
fn with_sequence(seq: &[(char, char)]) -> Stats {
    let store = Box::new(MemoryStore::new()) as Box<dyn EventStore>;
    let mut stats = Stats::new(store);
    let l = fixtures::layout("l", &"a".repeat(64));
    let k = fixtures::keyboard("k", &"b".repeat(64));
    stats.begin_session(&l, &k, "drill", None, 0).unwrap();
    for (i, (expected, typed)) in seq.iter().enumerate() {
        stats.record(*expected, *typed, i as i64).unwrap();
    }
    stats
}

#[test]
fn raw_counts_match_expected_chars() {
    let seq = [('a', 'a'), ('a', 'x'), ('b', 'b'), ('c', 'c'), ('c', 'c')];
    let stats = with_sequence(&seq);
    let raw = usage_map_raw(stats.store(), &EventFilter::default()).unwrap();
    assert_eq!(raw.get(&'a').copied(), Some(2));
    assert_eq!(raw.get(&'b').copied(), Some(1));
    assert_eq!(raw.get(&'c').copied(), Some(2));
}

#[test]
fn min_presses_filters_rare_keys() {
    // 'a' pressed once (below MIN_PRESSES = 3), 'b' pressed 5 times.
    let mut seq: Vec<(char, char)> = vec![('a', 'a')];
    seq.extend(std::iter::repeat(('b', 'b')).take(5));
    let stats = with_sequence(&seq);
    let map = usage_map(stats.store(), &EventFilter::default()).unwrap();
    assert!(!map.contains_key(&'a'), "'a' with 1 press should be below MIN_PRESSES");
    assert!(map.contains_key(&'b'), "'b' with 5 presses should appear");
    // Sanity — the threshold constant is what this test is about.
    assert!(MIN_PRESSES >= 2, "MIN_PRESSES is the threshold being verified");
}

#[test]
fn top_rank_normalizes_to_one() {
    // 3 presses on 'a', 30 on 'b'. 'b' is top rank; should saturate
    // at 1.0 regardless of the magnitude gap to 'a'.
    let mut seq: Vec<(char, char)> = std::iter::repeat(('a', 'a')).take(3).collect();
    seq.extend(std::iter::repeat(('b', 'b')).take(30));
    let stats = with_sequence(&seq);
    let map = usage_map(stats.store(), &EventFilter::default()).unwrap();
    let a = map.get(&'a').copied().unwrap();
    let b = map.get(&'b').copied().unwrap();
    assert!((b - 1.0).abs() < 1e-6, "top-rank key should saturate at 1.0 (got {b})");
    assert!(a < b, "lower-rank key should be below top (got a={a}, b={b})");
}

#[test]
fn rank_is_volume_independent() {
    // Same shape of use, drastically different volumes — both
    // should produce identical rank-normalized maps.
    let tiny = {
        let seq: Vec<(char, char)> = [('a', 'a'), ('a', 'a'), ('a', 'a'),
                                      ('b', 'b'), ('b', 'b'), ('b', 'b'), ('b', 'b')].into();
        let stats = with_sequence(&seq);
        usage_map(stats.store(), &EventFilter::default()).unwrap()
    };
    let huge = {
        let mut seq: Vec<(char, char)> = std::iter::repeat(('a', 'a')).take(300).collect();
        seq.extend(std::iter::repeat(('b', 'b')).take(700));
        let stats = with_sequence(&seq);
        usage_map(stats.store(), &EventFilter::default()).unwrap()
    };
    assert_eq!(tiny.len(), huge.len());
    for (ch, v_tiny) in &tiny {
        let v_huge = huge.get(ch).copied().unwrap();
        assert!((v_tiny - v_huge).abs() < 1e-6,
            "rank should be volume-independent: '{ch}' tiny={v_tiny}, huge={v_huge}");
    }
}

#[test]
fn uppercase_folds_into_lowercase_entry() {
    // 'A' and 'a' should count toward the same bucket.
    let seq: Vec<(char, char)> = [('A', 'A'), ('a', 'a'), ('a', 'a')].into();
    let stats = with_sequence(&seq);
    let raw = usage_map_raw(stats.store(), &EventFilter::default()).unwrap();
    assert_eq!(raw.get(&'a').copied(), Some(3));
    assert!(!raw.contains_key(&'A'));
}

#[test]
fn filter_isolates_layout_hash() {
    // 'a' on layout1, 'b' on layout2. Filtering by layout1 should
    // only surface 'a'.
    let store = Box::new(MemoryStore::new()) as Box<dyn EventStore>;
    let mut stats = Stats::new(store);
    let l1 = fixtures::layout("l1", &"a".repeat(64));
    let l2 = fixtures::layout("l2", &"c".repeat(64));
    let k = fixtures::keyboard("k", &"b".repeat(64));

    stats.begin_session(&l1, &k, "drill", None, 0).unwrap();
    for i in 0..5 {
        stats.record('a', 'a', i).unwrap();
    }
    stats.end_session(10).unwrap();

    stats.begin_session(&l2, &k, "drill", None, 20).unwrap();
    for i in 0..5 {
        stats.record('b', 'b', 20 + i).unwrap();
    }
    stats.end_session(30).unwrap();

    let filter1 = EventFilter {
        layout_hash: Some(l1.hash.clone()),
        ..Default::default()
    };
    let m1 = usage_map(stats.store(), &filter1).unwrap();
    assert_eq!(m1.len(), 1);
    assert!(m1.contains_key(&'a'));

    let filter2 = EventFilter {
        layout_hash: Some(l2.hash.clone()),
        ..Default::default()
    };
    let m2 = usage_map(stats.store(), &filter2).unwrap();
    assert_eq!(m2.len(), 1);
    assert!(m2.contains_key(&'b'));
}
