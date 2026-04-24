//! Integration tests for `views::heat`.
//!
//! The heat view is the first migration target from the old
//! `src/stats/` module. These tests encode the heat rule one more
//! time in a tiny reference oracle (`OracleHeat`) and compare the
//! view's output against it for a handful of event sequences.
//! If someone later tweaks the heat model in views/heat.rs, the
//! oracle here must be updated in lockstep — that's the point.

mod common;

use std::collections::HashMap;

use keywiz_stats::store::memory::MemoryStore;
use keywiz_stats::views::heat::{heat_map, MAX_HEAT, COOL_COST};
use keywiz_stats::{EventFilter, EventStore, Stats};

use common::fixtures;

/// Reference implementation of the heat model. Mirrors
/// `KeyRecord::update_heat` from keywiz's old `src/stats/`.
#[derive(Default, Clone, Copy)]
struct OracleKey {
    heat: u32,
    cooling_progress: u32,
}

impl OracleKey {
    fn apply(&mut self, correct: bool) {
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
    fn normalized(&self) -> f32 {
        self.heat as f32 / MAX_HEAT as f32
    }
}

#[derive(Default)]
struct Oracle {
    keys: HashMap<char, OracleKey>,
}

impl Oracle {
    fn record(&mut self, expected: char, correct: bool) {
        let key = expected.to_ascii_lowercase();
        self.keys.entry(key).or_default().apply(correct);
    }
    fn non_zero(&self) -> HashMap<char, f32> {
        self.keys
            .iter()
            .filter(|(_, r)| r.heat > 0)
            .map(|(c, r)| (*c, r.normalized()))
            .collect()
    }
}

/// Record a sequence of (expected, typed) pairs and return both
/// the oracle's heat map and the view's heat map.
fn record_and_query(
    seq: &[(char, char)],
) -> (HashMap<char, f32>, HashMap<char, f32>) {
    let store = Box::new(MemoryStore::new()) as Box<dyn EventStore>;
    let mut stats = Stats::new(store);
    let l = fixtures::layout("l", &"a".repeat(64));
    let k = fixtures::keyboard("k", &"b".repeat(64));
    stats.begin_session(&l, &k, "drill", None, 0).unwrap();

    let mut oracle = Oracle::default();
    for (i, (expected, typed)) in seq.iter().enumerate() {
        oracle.record(*expected, expected == typed);
        stats.record(*expected, *typed, i as i64).unwrap();
    }
    let view = heat_map(stats.store(), &EventFilter::default()).unwrap();
    (oracle.non_zero(), view)
}

fn assert_maps_match(oracle: &HashMap<char, f32>, view: &HashMap<char, f32>) {
    assert_eq!(
        oracle.len(),
        view.len(),
        "key count mismatch — oracle {oracle:?}, view {view:?}"
    );
    for (ch, expected) in oracle {
        let got = view.get(ch).copied().unwrap_or(0.0);
        assert!(
            (got - expected).abs() < 1e-6,
            "heat for '{ch}': oracle={expected}, view={got}"
        );
    }
}

#[test]
fn single_miss_produces_one_step_of_heat() {
    let (oracle, view) = record_and_query(&[('a', 'b')]);
    assert_eq!(view.get(&'a').copied(), Some(1.0 / MAX_HEAT as f32));
    assert_maps_match(&oracle, &view);
}

#[test]
fn correct_key_never_enters_the_map() {
    let (oracle, view) = record_and_query(&[('a', 'a'), ('a', 'a')]);
    assert!(view.is_empty(), "a correct key has zero heat: {view:?}");
    assert_maps_match(&oracle, &view);
}

#[test]
fn heat_caps_at_max_heat() {
    // 25 misses on the same key — should cap at MAX_HEAT normalized = 1.0.
    let seq: Vec<(char, char)> = std::iter::repeat(('a', 'b')).take(25).collect();
    let (oracle, view) = record_and_query(&seq);
    assert_eq!(view.get(&'a').copied(), Some(1.0));
    assert_maps_match(&oracle, &view);
}

#[test]
fn two_corrects_cool_one_step() {
    // 1 miss → heat 1. 2 corrects → heat 0 (fully cooled, dropped from map).
    let (oracle, view) = record_and_query(&[('a', 'b'), ('a', 'a'), ('a', 'a')]);
    assert!(view.is_empty(), "heat should be 0 after 2 corrects: {view:?}");
    assert_maps_match(&oracle, &view);
}

#[test]
fn partial_cooling_does_not_drop_heat() {
    // 1 miss → heat 1. 1 correct → cooling_progress 1, heat still 1.
    let (oracle, view) = record_and_query(&[('a', 'b'), ('a', 'a')]);
    assert_eq!(view.get(&'a').copied(), Some(1.0 / MAX_HEAT as f32));
    assert_maps_match(&oracle, &view);
}

#[test]
fn uppercase_folds_into_lowercase_entry() {
    // One miss on 'A', one on 'a' — should accumulate on 'a'.
    let (oracle, view) = record_and_query(&[('A', 'x'), ('a', 'x')]);
    assert_eq!(view.len(), 1);
    assert_eq!(view.get(&'a').copied(), Some(2.0 / MAX_HEAT as f32));
    assert_maps_match(&oracle, &view);
}

#[test]
fn filter_isolates_layout_hash() {
    // Two sessions, two layouts. Miss on 'a' in layout1, miss on 'b'
    // in layout2. Query layout1 should only see 'a'.
    let store = Box::new(MemoryStore::new()) as Box<dyn EventStore>;
    let mut stats = Stats::new(store);
    let l1 = fixtures::layout("l1", &"a".repeat(64));
    let l2 = fixtures::layout("l2", &"c".repeat(64));
    let k = fixtures::keyboard("k", &"b".repeat(64));

    stats.begin_session(&l1, &k, "drill", None, 0).unwrap();
    stats.record('a', 'x', 0).unwrap();
    stats.end_session(10).unwrap();

    stats.begin_session(&l2, &k, "drill", None, 20).unwrap();
    stats.record('b', 'x', 20).unwrap();
    stats.end_session(30).unwrap();

    let filter1 = EventFilter {
        layout_hash: Some(l1.hash.clone()),
        ..Default::default()
    };
    let view1 = heat_map(stats.store(), &filter1).unwrap();
    assert_eq!(view1.len(), 1);
    assert!(view1.contains_key(&'a'));

    let filter2 = EventFilter {
        layout_hash: Some(l2.hash.clone()),
        ..Default::default()
    };
    let view2 = heat_map(stats.store(), &filter2).unwrap();
    assert_eq!(view2.len(), 1);
    assert!(view2.contains_key(&'b'));
}
