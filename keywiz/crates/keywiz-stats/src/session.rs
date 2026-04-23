//! Session identity and summaries.
//!
//! A *session* is one continuous run of an exercise against one
//! (layout, keyboard) pair. Sessions are bounded explicitly — the
//! engine calls `begin_session` when any of {layout, keyboard,
//! exercise category} changes, and `end_session` on shutdown.
//! Implicit gap-based session splitting is a fallback only, for
//! crash recovery.

use serde::{Deserialize, Serialize};

use crate::snapshot::{KeyboardHash, LayoutHash};

/// Stable numeric identifier for a session. Assigned at session
/// start; opaque to callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub i64);

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Denormalized per-session rollup, written by
/// [`EventStore::end_session`](crate::EventStore::end_session).
///
/// Stored so views can answer "last N sessions" queries without
/// re-aggregating the event stream each time. Hashes pin which
/// iteration of the layout and which keyboard the session ran
/// against — critical for the drifter-tuning use case where a
/// layout's content evolves but the name stays the same.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: SessionId,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,

    /// Content hash of the layout snapshot in use.
    pub layout_hash: LayoutHash,
    /// Human label at session time (the layout's name). Kept for
    /// display; the hash is the authoritative identity.
    pub layout_name: String,

    /// Content hash of the keyboard snapshot in use.
    pub keyboard_hash: KeyboardHash,
    pub keyboard_name: String,

    /// `"drill"` / `"words"` / `"text"` / future categories.
    pub exercise_category: String,
    /// Drill level name, text passage title, etc. `None` when the
    /// category has no meaningful sub-selection.
    pub exercise_instance: Option<String>,

    /// Total keystrokes recorded in this session.
    pub total_events: u64,
    /// Keystrokes where `typed == expected`.
    pub total_correct: u64,
}
