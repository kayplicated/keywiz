//! The [`Stats`] facade — the surface the engine talks to.
//!
//! Wraps a boxed [`EventStore`], tracks the current session and
//! last-keystroke timestamp (for `delta_ms`), and exposes exactly
//! the operations keywiz's engine needs. Views consume the store
//! directly via [`Stats::store`].
//!
//! The facade owns:
//! - the store
//! - the active session id (once begun)
//! - the last-keystroke timestamp (for inter-keystroke deltas)
//!
//! It does *not* own any cached heat / rollup state — views are
//! recomputed on demand from the event stream. That way a bug in
//! one view can't poison another.

use anyhow::{Result, anyhow};

use crate::event::Event;
use crate::session::{SessionId, SessionSummary};
use crate::snapshot::{KeyboardSnapshot, LayoutSnapshot};
use crate::store::EventStore;

/// Maximum inter-keystroke gap that still counts as "typing".
/// Past this, `delta_ms` is reported as `None` and the view layer
/// should treat the pause as idle / AFK.
pub const IDLE_THRESHOLD_MS: u32 = 10_000;

/// Engine-facing stats facade.
pub struct Stats {
    store: Box<dyn EventStore>,
    session: Option<ActiveSession>,
}

struct ActiveSession {
    id: SessionId,
    last_ts_ms: Option<i64>,
}

impl Stats {
    /// Wrap an [`EventStore`] into a facade. The returned `Stats`
    /// has no active session; the caller must call
    /// [`begin_session`](Self::begin_session) before recording.
    pub fn new(store: Box<dyn EventStore>) -> Self {
        Self {
            store,
            session: None,
        }
    }

    /// Open a session. Idempotent-ish: if a session is already
    /// active, it's ended first with `started_at_ms` as the close
    /// time. Consumers that care about clean close semantics should
    /// call [`end_session`](Self::end_session) themselves.
    pub fn begin_session(
        &mut self,
        layout: &LayoutSnapshot,
        keyboard: &KeyboardSnapshot,
        exercise_category: &str,
        exercise_instance: Option<&str>,
        started_at_ms: i64,
    ) -> Result<SessionId> {
        if let Some(active) = self.session.take() {
            // Previous session wasn't explicitly closed — close it
            // now so the events don't dangle.
            let _ = self.store.end_session(active.id, started_at_ms);
        }
        let id = self.store.begin_session(
            layout,
            keyboard,
            exercise_category,
            exercise_instance,
            started_at_ms,
        )?;
        self.session = Some(ActiveSession { id, last_ts_ms: None });
        Ok(id)
    }

    /// Record one keystroke against the active session.
    ///
    /// Errors if no session is active. `delta_ms` is computed from
    /// the previous keystroke's timestamp; exceeds
    /// [`IDLE_THRESHOLD_MS`] collapse to `None`.
    pub fn record(
        &mut self,
        expected: char,
        typed: char,
        ts_ms: i64,
    ) -> Result<()> {
        let active = self
            .session
            .as_mut()
            .ok_or_else(|| anyhow!("record called with no active session"))?;
        let delta_ms = active.last_ts_ms.and_then(|prev| {
            let gap = ts_ms.saturating_sub(prev);
            if gap >= 0 && gap <= IDLE_THRESHOLD_MS as i64 {
                Some(gap as u32)
            } else {
                None
            }
        });
        active.last_ts_ms = Some(ts_ms);
        let event = Event {
            session_id: active.id,
            ts_ms,
            expected,
            typed,
            correct: expected == typed,
            delta_ms,
        };
        self.store.record(&event)
    }

    /// Close the active session. Returns the finalized summary.
    /// Errors if no session is active.
    pub fn end_session(&mut self, ended_at_ms: i64) -> Result<SessionSummary> {
        let active = self
            .session
            .take()
            .ok_or_else(|| anyhow!("end_session called with no active session"))?;
        self.store.end_session(active.id, ended_at_ms)
    }

    /// Active session id, if any. Useful for views scoped to "the
    /// current run" (in-session overlay).
    pub fn current_session(&self) -> Option<SessionId> {
        self.session.as_ref().map(|a| a.id)
    }

    /// Borrow the underlying store. Views read through this handle.
    pub fn store(&self) -> &dyn EventStore {
        self.store.as_ref()
    }
}
