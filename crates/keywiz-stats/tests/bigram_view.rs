//! Integration tests for `views::bigram` against a live store.
//!
//! The unit tests in the view file cover the derivation methods
//! (`miss_rate`, `avg_delta_ms`) in isolation. These integration
//! tests exercise the full flow: record events through the
//! `Stats` facade, query the view, assert pair semantics.

mod common;

use keywiz_stats::store::memory::MemoryStore;
use keywiz_stats::views::bigram::{bigram_stats, worst_bigrams};
use keywiz_stats::{EventFilter, EventStore, Stats};

use common::fixtures;

/// Shared setup: a fresh `Stats` with one open session, recording
/// the given `(expected, typed)` sequence.
fn recorded(seq: &[(char, char)]) -> Stats {
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
fn single_pair_is_counted_once() {
    // Type "ab" — one bigram (a→b), both correct.
    let stats = recorded(&[('a', 'a'), ('b', 'b')]);
    let map = bigram_stats(stats.store(), &EventFilter::default()).unwrap();
    assert_eq!(map.len(), 1);
    let b = map.get(&('a', 'b')).unwrap();
    assert_eq!(b.count, 1);
    assert_eq!(b.miss_count, 0);
}

#[test]
fn miss_on_second_is_the_bigrams_miss() {
    // Type t-expecting-h but user types x. The bigram (t, h)
    // records a miss because the transition's destination failed.
    let stats = recorded(&[('t', 't'), ('h', 'x')]);
    let map = bigram_stats(stats.store(), &EventFilter::default()).unwrap();
    let b = map.get(&('t', 'h')).unwrap();
    assert_eq!(b.count, 1);
    assert_eq!(b.miss_count, 1);
    assert!((b.miss_rate() - 1.0).abs() < 1e-9);
}

#[test]
fn overlapping_pairs_tally_correctly() {
    // Type "abc" — two bigrams: (a,b) and (b,c). Each counted once.
    let stats = recorded(&[('a', 'a'), ('b', 'b'), ('c', 'c')]);
    let map = bigram_stats(stats.store(), &EventFilter::default()).unwrap();
    assert_eq!(map.len(), 2);
    assert_eq!(map.get(&('a', 'b')).unwrap().count, 1);
    assert_eq!(map.get(&('b', 'c')).unwrap().count, 1);
}

#[test]
fn pairs_never_cross_session_boundaries() {
    // Two sessions of "a, b" each. A naive cross-session walk
    // would see a→b, b→a, a→b for 3 pairs; per-session walking
    // sees only a→b twice.
    let store = Box::new(MemoryStore::new()) as Box<dyn EventStore>;
    let mut stats = Stats::new(store);
    let l = fixtures::layout("l", &"a".repeat(64));
    let k = fixtures::keyboard("k", &"b".repeat(64));

    stats.begin_session(&l, &k, "drill", None, 0).unwrap();
    stats.record('a', 'a', 0).unwrap();
    stats.record('b', 'b', 10).unwrap();
    stats.end_session(20).unwrap();

    stats.begin_session(&l, &k, "drill", None, 100).unwrap();
    stats.record('a', 'a', 100).unwrap();
    stats.record('b', 'b', 110).unwrap();
    stats.end_session(120).unwrap();

    let map = bigram_stats(stats.store(), &EventFilter::default()).unwrap();
    // Two (a,b) pairs — one per session — and nothing else.
    assert_eq!(map.len(), 1);
    assert_eq!(map.get(&('a', 'b')).unwrap().count, 2);
}

#[test]
fn case_folds_into_lowercase() {
    // Typing capitals should accumulate on the lowercase pair.
    let stats = recorded(&[('A', 'A'), ('B', 'B'), ('a', 'a'), ('b', 'b')]);
    let map = bigram_stats(stats.store(), &EventFilter::default()).unwrap();
    // Four events = 3 consecutive pairs, all collapse to (a,b)/(b,a)
    // after lowercasing: (A,B) -> (a,b), (B,a) -> (b,a), (a,b).
    assert_eq!(map.get(&('a', 'b')).unwrap().count, 2);
    assert_eq!(map.get(&('b', 'a')).unwrap().count, 1);
}

#[test]
fn worst_bigrams_orders_by_miss_rate() {
    // Three bigrams: ab perfect (5/5), cd moderate (2/4), ef bad (4/5).
    // Cover each with enough repetitions to exceed the min_count gate.
    let mut seq = Vec::new();
    // ab — 5 occurrences of the (a,b) pair (10 events).
    for _ in 0..5 {
        seq.push(('a', 'a'));
        seq.push(('b', 'b'));
    }
    // Force session boundary so we don't create spurious (b,c) pairs.
    let stats_ab = recorded(&seq);

    // Skip — need multiple bigrams in one session. Redo cleanly:
    let store = Box::new(MemoryStore::new()) as Box<dyn EventStore>;
    let mut stats = Stats::new(store);
    let l = fixtures::layout("l", &"a".repeat(64));
    let k = fixtures::keyboard("k", &"b".repeat(64));

    // Session 1: ab pairs, all correct (5 repetitions).
    stats.begin_session(&l, &k, "drill", None, 0).unwrap();
    for i in 0..5 {
        stats.record('a', 'a', (i * 20) as i64).unwrap();
        stats.record('b', 'b', (i * 20 + 10) as i64).unwrap();
    }
    stats.end_session(200).unwrap();

    // Session 2: cd pairs, half miss (4 repetitions, 2 misses on d).
    stats.begin_session(&l, &k, "drill", None, 300).unwrap();
    for i in 0..4 {
        stats.record('c', 'c', 300 + (i * 20) as i64).unwrap();
        let d_input = if i < 2 { 'x' } else { 'd' };
        stats.record('d', d_input, 300 + (i * 20 + 10) as i64).unwrap();
    }
    stats.end_session(500).unwrap();

    // Session 3: ef pairs, 4/5 miss (5 repetitions, 4 misses on f).
    stats.begin_session(&l, &k, "drill", None, 600).unwrap();
    for i in 0..5 {
        stats.record('e', 'e', 600 + (i * 20) as i64).unwrap();
        let f_input = if i < 4 { 'x' } else { 'f' };
        stats.record('f', f_input, 600 + (i * 20 + 10) as i64).unwrap();
    }
    stats.end_session(800).unwrap();

    // Repeating "ab ab ab ..." within a session produces pairs
    // (a,b), (b,a), (a,b), (b,a), ... — that's a real bigram each
    // time the *event stream* has two consecutive chars, regardless
    // of semantic meaning. Filter to just the (X, Y) pairs we seeded
    // intentionally for the ranking assertion.
    let ranked = worst_bigrams(stats.store(), &EventFilter::default(), 4).unwrap();
    let target: Vec<((char, char), _)> = ranked
        .into_iter()
        .filter(|(k, _)| matches!(k, ('a', 'b') | ('c', 'd') | ('e', 'f')))
        .collect();
    let keys: Vec<(char, char)> = target.iter().map(|(k, _)| *k).collect();
    // ef (0.8), cd (0.5), ab (0.0) — descending miss rate.
    assert_eq!(keys, vec![('e', 'f'), ('c', 'd'), ('a', 'b')]);

    // Use stats_ab so the compiler doesn't warn.
    let _ = stats_ab;
}
