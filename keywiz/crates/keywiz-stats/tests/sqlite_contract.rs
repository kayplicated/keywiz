//! Run the shared contract suite against `SqliteStore`.
//!
//! Uses an in-memory sqlite connection per test so state never
//! leaks across cases and the filesystem stays clean.

mod common;

use keywiz_stats::EventStore;
use keywiz_stats::store::sqlite::SqliteStore;

fn make_sqlite() -> Box<dyn EventStore> {
    Box::new(SqliteStore::open_in_memory().expect("open in-memory sqlite"))
}

macro_rules! contract_cases {
    ($factory:expr, $($name:ident),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                common::contract::$name($factory);
            }
        )*
    };
}

contract_cases!(
    make_sqlite,
    begin_session_allocates_distinct_ids,
    begin_session_upserts_snapshots_without_dupe,
    record_rejects_unknown_session,
    end_session_totals_match_recorded_events,
    events_filter_by_session_id,
    events_filter_by_correct,
    events_filter_by_time_range,
    events_filter_by_layout_hash_joins_through_session,
    sessions_are_reverse_chronological,
    sessions_filter_isolates_layout_hash,
    snapshot_roundtrip_by_hash,
    facade_delta_ms_first_is_none_then_some,
    facade_delta_ms_collapses_after_idle,
    facade_record_without_session_errors,
);

// SQLite-specific: persistence actually persists.
#[test]
fn sqlite_roundtrip_across_reopen() {
    use keywiz_stats::{EventFilter, SessionFilter, Stats};

    let tmp = tempdir_path();
    let path = tmp.join("stats.sqlite");

    // First connection: begin, record, end.
    {
        let store = SqliteStore::open(&path).expect("open #1");
        let mut stats = Stats::new(Box::new(store));
        let l = common::fixtures::layout("drifter", &"a".repeat(64));
        let k = common::fixtures::keyboard("elora", &"b".repeat(64));
        stats.begin_session(&l, &k, "drill", None, 1000).unwrap();
        stats.record('a', 'a', 1000).unwrap();
        stats.record('b', 'x', 1100).unwrap();
        stats.end_session(1200).unwrap();
    }

    // Second connection: sees the persisted session + events.
    {
        let store = SqliteStore::open(&path).expect("open #2");
        let sessions = store.sessions(&SessionFilter::default()).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].total_events, 2);
        assert_eq!(sessions[0].total_correct, 1);
        assert_eq!(sessions[0].ended_at_ms, Some(1200));

        let events: Vec<_> = store
            .events(&EventFilter::default())
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(events.len(), 2);
    }

    std::fs::remove_dir_all(&tmp).ok();
}

fn tempdir_path() -> std::path::PathBuf {
    // Minimal test-local tempdir — avoids pulling in the tempfile
    // crate for one call site.
    let mut p = std::env::temp_dir();
    let nonce = format!(
        "keywiz-stats-test-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    p.push(nonce);
    std::fs::create_dir_all(&p).expect("create tempdir");
    p
}
