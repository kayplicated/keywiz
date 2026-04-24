//! Contract tests any [`EventStore`] impl must satisfy.
//!
//! The `Factory` indirection means we can plug `MemoryStore` or
//! `SqliteStore` (or any future backend) into the same test bodies.
//! If you break the contract, both stores go red simultaneously and
//! you know the fault is in the trait's semantics, not one impl's
//! quirk.

// Rust compiles this file once per integration-test binary. Tests
// that don't happen to call every helper (e.g. the heat_view
// binary) would otherwise surface spurious dead-code warnings.
#![allow(dead_code)]

use keywiz_stats::{
    Event, EventFilter, EventStore, IDLE_THRESHOLD_MS, SessionFilter, Stats,
};

use super::fixtures;

pub type Factory = fn() -> Box<dyn EventStore>;

// ---- direct-store tests ----

pub fn begin_session_allocates_distinct_ids(make: Factory) {
    let mut store = make();
    let l = fixtures::layout("drifter", "a".repeat(64).as_str());
    let k = fixtures::keyboard("elora", "b".repeat(64).as_str());
    let id1 = store.begin_session(&l, &k, "drill", None, 1000).unwrap();
    let id2 = store.begin_session(&l, &k, "drill", None, 2000).unwrap();
    assert_ne!(id1, id2, "each begin_session must allocate a fresh id");
}

pub fn begin_session_upserts_snapshots_without_dupe(make: Factory) {
    let mut store = make();
    let l = fixtures::layout("drifter", &"a".repeat(64));
    let k = fixtures::keyboard("elora", &"b".repeat(64));
    store.begin_session(&l, &k, "drill", None, 1000).unwrap();
    store.begin_session(&l, &k, "drill", None, 2000).unwrap();

    // Re-inserting the same hash must not produce duplicates or
    // errors; both hashes still resolve cleanly.
    let got_layout = store.layout_snapshot(&l.hash).unwrap();
    let got_keyboard = store.keyboard_snapshot(&k.hash).unwrap();
    assert!(got_layout.is_some());
    assert!(got_keyboard.is_some());
}

pub fn record_rejects_unknown_session(make: Factory) {
    let mut store = make();
    let bogus = Event {
        session_id: keywiz_stats::SessionId(9999),
        ts_ms: 0,
        expected: 'a',
        typed: 'a',
        correct: true,
        delta_ms: None,
    };
    assert!(store.record(&bogus).is_err());
}

pub fn end_session_totals_match_recorded_events(make: Factory) {
    let mut store = make();
    let l = fixtures::layout("l", &"a".repeat(64));
    let k = fixtures::keyboard("k", &"b".repeat(64));
    let id = store.begin_session(&l, &k, "drill", None, 0).unwrap();

    // 3 correct, 2 incorrect.
    for (i, (e, t)) in [
        ('a', 'a'),
        ('b', 'x'),
        ('c', 'c'),
        ('d', 'y'),
        ('e', 'e'),
    ]
    .iter()
    .enumerate()
    {
        store
            .record(&Event {
                session_id: id,
                ts_ms: 100 * i as i64,
                expected: *e,
                typed: *t,
                correct: e == t,
                delta_ms: None,
            })
            .unwrap();
    }
    let summary = store.end_session(id, 1000).unwrap();
    assert_eq!(summary.total_events, 5);
    assert_eq!(summary.total_correct, 3);
    assert_eq!(summary.ended_at_ms, Some(1000));
}

pub fn events_filter_by_session_id(make: Factory) {
    let mut store = make();
    let l = fixtures::layout("l", &"a".repeat(64));
    let k = fixtures::keyboard("k", &"b".repeat(64));
    let id1 = store.begin_session(&l, &k, "drill", None, 0).unwrap();
    let id2 = store.begin_session(&l, &k, "drill", None, 1000).unwrap();
    for (sid, ch) in &[(id1, 'x'), (id2, 'y'), (id1, 'z')] {
        store
            .record(&Event {
                session_id: *sid,
                ts_ms: 0,
                expected: *ch,
                typed: *ch,
                correct: true,
                delta_ms: None,
            })
            .unwrap();
    }
    let f = EventFilter {
        session_id: Some(id1),
        ..Default::default()
    };
    let got: Vec<_> = store
        .events(&f)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(got.len(), 2);
    assert!(got.iter().all(|e| e.session_id == id1));
}

pub fn events_filter_by_correct(make: Factory) {
    let mut store = make();
    let l = fixtures::layout("l", &"a".repeat(64));
    let k = fixtures::keyboard("k", &"b".repeat(64));
    let id = store.begin_session(&l, &k, "drill", None, 0).unwrap();
    for (i, correct) in [true, false, true, false, false].iter().enumerate() {
        store
            .record(&Event {
                session_id: id,
                ts_ms: i as i64,
                expected: 'a',
                typed: if *correct { 'a' } else { 'b' },
                correct: *correct,
                delta_ms: None,
            })
            .unwrap();
    }
    let miss = EventFilter {
        correct: Some(false),
        ..Default::default()
    };
    let misses: Vec<_> = store
        .events(&miss)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(misses.len(), 3, "three misses recorded");
    assert!(misses.iter().all(|e| !e.correct));
}

pub fn events_filter_by_time_range(make: Factory) {
    let mut store = make();
    let l = fixtures::layout("l", &"a".repeat(64));
    let k = fixtures::keyboard("k", &"b".repeat(64));
    let id = store.begin_session(&l, &k, "drill", None, 0).unwrap();
    for ts in [100, 200, 300, 400, 500] {
        store
            .record(&Event {
                session_id: id,
                ts_ms: ts,
                expected: 'a',
                typed: 'a',
                correct: true,
                delta_ms: None,
            })
            .unwrap();
    }
    let f = EventFilter {
        from_ms: Some(200),
        until_ms: Some(400),
        ..Default::default()
    };
    let got: Vec<_> = store
        .events(&f)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(got.len(), 3, "inclusive range 200..=400");
    assert!(got.iter().all(|e| (200..=400).contains(&e.ts_ms)));
}

pub fn events_filter_by_layout_hash_joins_through_session(make: Factory) {
    let mut store = make();
    let l1 = fixtures::layout("drifter", &"a".repeat(64));
    let l2 = fixtures::layout("gallium", &"c".repeat(64));
    let k = fixtures::keyboard("elora", &"b".repeat(64));
    let id1 = store.begin_session(&l1, &k, "drill", None, 0).unwrap();
    let id2 = store.begin_session(&l2, &k, "drill", None, 1000).unwrap();
    store
        .record(&Event {
            session_id: id1,
            ts_ms: 0,
            expected: 'a',
            typed: 'a',
            correct: true,
            delta_ms: None,
        })
        .unwrap();
    store
        .record(&Event {
            session_id: id2,
            ts_ms: 0,
            expected: 'b',
            typed: 'b',
            correct: true,
            delta_ms: None,
        })
        .unwrap();

    let f = EventFilter {
        layout_hash: Some(l1.hash.clone()),
        ..Default::default()
    };
    let got: Vec<_> = store
        .events(&f)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].session_id, id1);
}

pub fn sessions_are_reverse_chronological(make: Factory) {
    let mut store = make();
    let l = fixtures::layout("l", &"a".repeat(64));
    let k = fixtures::keyboard("k", &"b".repeat(64));
    let id_early = store.begin_session(&l, &k, "drill", None, 100).unwrap();
    let id_mid = store.begin_session(&l, &k, "drill", None, 500).unwrap();
    let id_late = store.begin_session(&l, &k, "drill", None, 900).unwrap();

    let rows = store.sessions(&SessionFilter::default()).unwrap();
    let ids: Vec<_> = rows.iter().map(|s| s.session_id).collect();
    assert_eq!(ids, vec![id_late, id_mid, id_early]);
}

pub fn sessions_filter_isolates_layout_hash(make: Factory) {
    let mut store = make();
    let l1 = fixtures::layout("drifter", &"a".repeat(64));
    let l2 = fixtures::layout("drifter-v2", &"c".repeat(64));
    let k = fixtures::keyboard("elora", &"b".repeat(64));
    let id_v1a = store.begin_session(&l1, &k, "drill", None, 100).unwrap();
    let _id_v2 = store.begin_session(&l2, &k, "drill", None, 200).unwrap();
    let id_v1b = store.begin_session(&l1, &k, "drill", None, 300).unwrap();

    let f = SessionFilter {
        layout_hash: Some(l1.hash.clone()),
        ..Default::default()
    };
    let rows = store.sessions(&f).unwrap();
    let ids: Vec<_> = rows.iter().map(|s| s.session_id).collect();
    assert_eq!(ids, vec![id_v1b, id_v1a]);
}

pub fn snapshot_roundtrip_by_hash(make: Factory) {
    let mut store = make();
    let l = fixtures::layout("drifter", &"a".repeat(64));
    let k = fixtures::keyboard("elora", &"b".repeat(64));
    store.begin_session(&l, &k, "drill", None, 0).unwrap();

    let got_l = store.layout_snapshot(&l.hash).unwrap().unwrap();
    let got_k = store.keyboard_snapshot(&k.hash).unwrap().unwrap();
    assert_eq!(got_l.name, "drifter");
    assert_eq!(got_l.canonical_json, l.canonical_json);
    assert_eq!(got_k.name, "elora");
    assert_eq!(got_k.canonical_json, k.canonical_json);
}

// ---- facade tests ----

pub fn facade_delta_ms_first_is_none_then_some(make: Factory) {
    let mut stats = Stats::new(make());
    let l = fixtures::layout("l", &"a".repeat(64));
    let k = fixtures::keyboard("k", &"b".repeat(64));
    let session_id = stats.begin_session(&l, &k, "drill", None, 1000).unwrap();

    stats.record('a', 'a', 1000).unwrap();
    stats.record('b', 'b', 1500).unwrap();
    stats.record('c', 'c', 1700).unwrap();

    // Collect all events for this session and assert delta_ms.
    let f = EventFilter {
        session_id: Some(session_id),
        ..Default::default()
    };
    let events: Vec<_> = stats
        .store()
        .events(&f)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].delta_ms, None, "first keystroke has no delta");
    assert_eq!(events[1].delta_ms, Some(500));
    assert_eq!(events[2].delta_ms, Some(200));
}

pub fn facade_delta_ms_collapses_after_idle(make: Factory) {
    let mut stats = Stats::new(make());
    let l = fixtures::layout("l", &"a".repeat(64));
    let k = fixtures::keyboard("k", &"b".repeat(64));
    let session_id = stats.begin_session(&l, &k, "drill", None, 0).unwrap();

    stats.record('a', 'a', 0).unwrap();
    // Gap exceeds IDLE_THRESHOLD_MS.
    stats
        .record('b', 'b', IDLE_THRESHOLD_MS as i64 + 1)
        .unwrap();

    let f = EventFilter {
        session_id: Some(session_id),
        ..Default::default()
    };
    let events: Vec<_> = stats
        .store()
        .events(&f)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(events[0].delta_ms, None);
    assert_eq!(
        events[1].delta_ms, None,
        "idle gap should collapse delta to None"
    );
}

pub fn facade_record_without_session_errors(make: Factory) {
    let mut stats = Stats::new(make());
    let err = stats.record('a', 'a', 0).unwrap_err();
    assert!(format!("{err:#}").contains("no active session"));
}

