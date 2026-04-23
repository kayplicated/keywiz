//! The `Event` record — one row per keystroke.
//!
//! The event stream is the source of truth for every stat keywiz
//! tracks. Views compute their output by filtering and aggregating
//! this stream. Denormalized fields (per-row layout/keyboard hashes)
//! are intentionally absent: those live on the session the event
//! belongs to.

use serde::{Deserialize, Serialize};

use crate::SessionId;

/// One keystroke.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Session this keystroke belongs to. Joined to [`Session`]
    /// for layout / keyboard / exercise context.
    ///
    /// [`Session`]: crate::session::SessionSummary
    pub session_id: SessionId,

    /// Millis since Unix epoch. Monotonic within a session; ordering
    /// across sessions is best-effort (wall-clock sensitive).
    pub ts_ms: i64,

    /// The character the exercise asked the user to type.
    pub expected: char,

    /// The character the user actually typed.
    pub typed: char,

    /// `typed == expected`, pre-computed so views don't re-derive.
    /// The raw chars are preserved so bigram miss-pattern views can
    /// see *what* was typed instead.
    pub correct: bool,

    /// Milliseconds since the previous keystroke in the same session.
    /// `None` for the session's first keystroke, or when the gap
    /// exceeded an implementation-defined threshold (idle / AFK).
    pub delta_ms: Option<u32>,
}
