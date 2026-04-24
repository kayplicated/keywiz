//! Live per-session numbers — the "correct / wrong / accuracy"
//! footer keywiz paints during a running session.
//!
//! Derived from the event stream rather than read from a
//! session-summary row, because the summary isn't finalized until
//! `end_session` fires. Callers typically pass `SessionId` from
//! `Stats::current_session()` and re-query once per render.
//!
//! Cheap: each event carries its own `correct` bool; one pass
//! tallies them.

use anyhow::Result;

use crate::session::SessionId;
use crate::{EventFilter, EventStore};

/// Rolling counts for an in-progress session.
#[derive(Debug, Clone, Copy, Default)]
pub struct SessionLive {
    pub total_events: u64,
    pub total_correct: u64,
    pub total_wrong: u64,
}

impl SessionLive {
    /// Accuracy as a 0.0..=1.0 float. Returns 1.0 on an empty
    /// session so the default "100%" display reads sensibly before
    /// the first keystroke.
    pub fn accuracy(&self) -> f64 {
        if self.total_events == 0 {
            return 1.0;
        }
        self.total_correct as f64 / self.total_events as f64
    }
}

/// Tally live numbers for `session_id`. No error on an unknown
/// session id — returns an empty `SessionLive` since "no events
/// recorded yet" and "session doesn't exist" collapse to the same
/// visible state in the UI.
pub fn live_for(store: &dyn EventStore, session_id: SessionId) -> Result<SessionLive> {
    let filter = EventFilter {
        session_id: Some(session_id),
        ..Default::default()
    };
    let mut live = SessionLive::default();
    for event in store.events(&filter)? {
        let event = event?;
        live.total_events += 1;
        if event.correct {
            live.total_correct += 1;
        } else {
            live.total_wrong += 1;
        }
    }
    Ok(live)
}

#[cfg(test)]
mod tests {
    // Not integration-tested here because SessionLive requires a
    // store with an active session — those tests live under
    // `tests/heat_view.rs`-style integration files which can pull
    // in fixtures + stats::Stats facade. A follow-up integration
    // test would assert:
    //   - empty session → total_events=0, accuracy=1.0
    //   - mixed hits/misses → correct ratio
    //   - events from other sessions are filtered out
}
