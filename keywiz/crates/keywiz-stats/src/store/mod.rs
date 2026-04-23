//! Persistence abstraction for the event stream.
//!
//! The [`EventStore`] trait is the one boundary storage crosses.
//! Views depend on the trait, not any concrete store. Two impls
//! planned:
//!
//! - [`sqlite`] — production, on-disk, bundled rusqlite.
//! - [`memory`] — in-memory, for tests and opt-out-of-persistence.
//!
//! Neither is implemented in this pass; this module defines the
//! shapes.

use anyhow::Result;

use crate::event::Event;
use crate::session::{SessionId, SessionSummary};
use crate::snapshot::{KeyboardHash, KeyboardSnapshot, LayoutHash, LayoutSnapshot};

pub mod memory;
pub mod sqlite;

/// Persistence + query surface for the event stream.
///
/// Writers (the engine) call [`begin_session`], [`record`],
/// [`end_session`] in order. Readers (views) call [`events`] and
/// [`sessions`] with filters to aggregate.
///
/// Stores own their own batching / flushing strategy but must not
/// lose writes on clean shutdown. Call sites treat writes as
/// best-effort (I/O failures log but don't crash typing) — the
/// return `Result` is for the store's internal consumption and
/// structured error reporting.
///
/// [`begin_session`]: Self::begin_session
/// [`record`]: Self::record
/// [`end_session`]: Self::end_session
/// [`events`]: Self::events
/// [`sessions`]: Self::sessions
pub trait EventStore: Send + Sync {
    // ---- session lifecycle ----

    /// Open a new session. Upserts the layout and keyboard
    /// snapshots first (no-op if the hash already exists), then
    /// allocates a fresh `SessionId`. Returns the id so the caller
    /// can stamp every subsequent [`Event::session_id`].
    fn begin_session(
        &mut self,
        layout: &LayoutSnapshot,
        keyboard: &KeyboardSnapshot,
        exercise_category: &str,
        exercise_instance: Option<&str>,
        started_at_ms: i64,
    ) -> Result<SessionId>;

    /// Close a session. Computes per-session rollups (total events,
    /// total correct, ended_at) from the event stream and writes
    /// them to the sessions table. Returns the final summary.
    fn end_session(&mut self, session_id: SessionId, ended_at_ms: i64) -> Result<SessionSummary>;

    // ---- writes ----

    /// Append one event. Cheap; store implementations may batch
    /// internally but must surface errors eventually.
    fn record(&mut self, event: &Event) -> Result<()>;

    // ---- reads ----

    /// Iterate events matching `filter`, in chronological order.
    /// Empty filter = every event. Views use the iterator form so
    /// large ranges don't have to materialize in one Vec.
    fn events<'a>(
        &'a self,
        filter: &EventFilter,
    ) -> Result<Box<dyn Iterator<Item = Result<Event>> + 'a>>;

    /// Query session summaries. Returned in reverse chronological
    /// order (most recent first) — views building "last N sessions"
    /// lists don't have to re-sort.
    fn sessions(&self, filter: &SessionFilter) -> Result<Vec<SessionSummary>>;

    // ---- snapshots (reads only; writes happen via begin_session) ----

    /// Fetch a layout snapshot by hash. `None` if not seen.
    fn layout_snapshot(&self, hash: &LayoutHash) -> Result<Option<LayoutSnapshot>>;

    /// Fetch a keyboard snapshot by hash.
    fn keyboard_snapshot(&self, hash: &KeyboardHash) -> Result<Option<KeyboardSnapshot>>;
}

/// Filter for [`EventStore::events`]. All fields are inclusive AND
/// predicates; `None` = no constraint on that axis.
#[derive(Debug, Default, Clone)]
pub struct EventFilter {
    pub session_id: Option<SessionId>,
    /// Restrict to events whose `session_id` is in this set. `None`
    /// = no constraint. Used by the stats UI to express "events
    /// from sessions matching a (layout_name, keyboard_name) combo"
    /// without the store needing to know how the caller derived
    /// the set. An empty `Vec` matches zero events — the caller's
    /// "no sessions found" state propagates naturally.
    pub session_ids: Option<Vec<SessionId>>,
    pub layout_hash: Option<LayoutHash>,
    pub keyboard_hash: Option<KeyboardHash>,
    pub exercise_category: Option<String>,
    pub from_ms: Option<i64>,
    pub until_ms: Option<i64>,
    /// Only events where `correct == Some(v)`. `None` = include both.
    pub correct: Option<bool>,
}

/// Filter for [`EventStore::sessions`]. Same AND semantics as
/// [`EventFilter`].
#[derive(Debug, Default, Clone)]
pub struct SessionFilter {
    pub layout_hash: Option<LayoutHash>,
    pub layout_name: Option<String>,
    pub keyboard_hash: Option<KeyboardHash>,
    pub exercise_category: Option<String>,
    pub from_ms: Option<i64>,
    pub until_ms: Option<i64>,
    /// Cap the result count. `None` = unlimited.
    pub limit: Option<usize>,
}
