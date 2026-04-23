//! Run the shared contract suite against `MemoryStore`.

mod common;

use keywiz_stats::EventStore;
use keywiz_stats::store::memory::MemoryStore;

fn make_memory() -> Box<dyn EventStore> {
    Box::new(MemoryStore::new())
}

// Each contract case becomes its own `#[test]` so failures are
// reported individually. Identical pattern will repeat in
// sqlite_contract.rs once that store lands.

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
    make_memory,
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
