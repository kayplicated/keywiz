//! Integration tests for `views::keys` against a live store.
//!
//! Unit tests in the view file cover `miss_rate` / `avg_delta_ms`
//! on hand-built `KeyStats`. These tests exercise `key_stats` and
//! `worst_keys` end-to-end: record events through `Stats`, query
//! the view, assert per-key grouping / case folding / sort order.

mod common;

use keywiz_stats::store::memory::MemoryStore;
use keywiz_stats::views::keys::{key_stats, worst_keys};
use keywiz_stats::{EventFilter, EventStore, Stats};

use common::fixtures;

/// Fresh `Stats` with one session, recording the given
/// `(expected, typed)` sequence at consecutive ts.
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
fn each_expected_char_becomes_its_own_bucket() {
    let stats = recorded(&[('a', 'a'), ('b', 'b'), ('c', 'c')]);
    let map = key_stats(stats.store(), &EventFilter::default()).unwrap();
    assert_eq!(map.len(), 3);
    assert_eq!(map.get(&'a').unwrap().count, 1);
    assert_eq!(map.get(&'b').unwrap().count, 1);
    assert_eq!(map.get(&'c').unwrap().count, 1);
}

#[test]
fn miss_attributes_to_expected_not_typed() {
    // User was asked for 'a' and typed 'x'. Miss attaches to 'a',
    // not 'x' — the view asks "how often did I *miss* an 'a'," not
    // "how often did I *type* an x."
    let stats = recorded(&[('a', 'x')]);
    let map = key_stats(stats.store(), &EventFilter::default()).unwrap();
    assert_eq!(map.len(), 1);
    let a = map.get(&'a').unwrap();
    assert_eq!(a.count, 1);
    assert_eq!(a.miss_count, 1);
    assert!(map.get(&'x').is_none());
}

#[test]
fn case_folds_to_lowercase() {
    // Typing 'A' (uppercase), 'a' (lowercase) — both accumulate
    // into the same 'a' bucket.
    let stats = recorded(&[('A', 'A'), ('a', 'a'), ('A', 'a')]);
    let map = key_stats(stats.store(), &EventFilter::default()).unwrap();
    assert_eq!(map.len(), 1);
    let a = map.get(&'a').unwrap();
    assert_eq!(a.count, 3);
    // The last entry was expected='A', typed='a' → case-sensitive
    // compare says mismatch. Our recording already computed
    // `correct == (expected == typed)` at record time, so it's a
    // miss.
    assert_eq!(a.miss_count, 1);
}

#[test]
fn worst_keys_respects_min_count_threshold() {
    // 'a' missed once out of one (100% miss); 'b' missed twice out
    // of three (66%); 'c' hit three out of three (0%). With
    // min_count=2, 'a' is filtered out despite its 100% miss rate
    // — single-sample flukes shouldn't top the list.
    let stats = recorded(&[
        ('a', 'x'),
        ('b', 'x'),
        ('b', 'x'),
        ('b', 'b'),
        ('c', 'c'),
        ('c', 'c'),
        ('c', 'c'),
    ]);
    let sorted = worst_keys(stats.store(), &EventFilter::default(), 2).unwrap();
    assert_eq!(sorted.len(), 2, "a excluded by min_count");
    assert_eq!(sorted[0].0, 'b');
    assert!((sorted[0].1.miss_rate() - (2.0 / 3.0)).abs() < 1e-9);
    assert_eq!(sorted[1].0, 'c');
    assert_eq!(sorted[1].1.miss_rate(), 0.0);
}

#[test]
fn worst_keys_orders_by_miss_rate_then_count() {
    // 'a': 2/4 miss = 50%. 'b': 2/4 miss = 50%. Tie on rate →
    // order by count descending; tie there too → either order
    // stable. We assert both appear in the top 2 at 50% rate.
    let stats = recorded(&[
        ('a', 'x'),
        ('a', 'x'),
        ('a', 'a'),
        ('a', 'a'),
        ('b', 'x'),
        ('b', 'x'),
        ('b', 'b'),
        ('b', 'b'),
    ]);
    let sorted = worst_keys(stats.store(), &EventFilter::default(), 1).unwrap();
    assert_eq!(sorted.len(), 2);
    assert!((sorted[0].1.miss_rate() - 0.5).abs() < 1e-9);
    assert!((sorted[1].1.miss_rate() - 0.5).abs() < 1e-9);
}

#[test]
fn empty_store_yields_empty_map() {
    let store = Box::new(MemoryStore::new()) as Box<dyn EventStore>;
    let stats = Stats::new(store);
    let map = key_stats(stats.store(), &EventFilter::default()).unwrap();
    assert!(map.is_empty());
    let sorted = worst_keys(stats.store(), &EventFilter::default(), 1).unwrap();
    assert!(sorted.is_empty());
}
