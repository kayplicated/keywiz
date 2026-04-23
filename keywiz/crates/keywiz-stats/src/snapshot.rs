//! Content-addressed layout and keyboard snapshots.
//!
//! Sessions reference their layout and keyboard by content hash,
//! not name. Two sessions running "drifter" after a key swap have
//! different `LayoutHash`es and are counted as different iterations.
//! Two sessions with the same hash but different recorded dates are
//! the same iteration — even if the filename was renamed.
//!
//! The hash function, canonical serialization, and the conversion
//! from `drift_core` / `keywiz` types live in [`crate::hash`].

use serde::{Deserialize, Serialize};

/// SHA-256 of the canonicalized layout JSON. Hex-encoded for
/// readability and stable across platforms.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LayoutHash(pub String);

/// SHA-256 of the canonicalized keyboard JSON.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyboardHash(pub String);

impl std::fmt::Display for LayoutHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::fmt::Display for KeyboardHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// A full layout snapshot. First-seen-per-hash is written into the
/// store; subsequent sessions with the same hash reuse the existing
/// row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutSnapshot {
    pub hash: LayoutHash,
    pub name: String,
    /// Canonicalized JSON — the exact bytes the hash was computed
    /// over. Views that want to inspect an iteration's shape (e.g.
    /// layout-diff between two hashes) read this.
    pub canonical_json: String,
    pub first_seen_ms: i64,
}

/// A full keyboard snapshot. Same content-address semantics as
/// [`LayoutSnapshot`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardSnapshot {
    pub hash: KeyboardHash,
    pub name: String,
    pub canonical_json: String,
    pub first_seen_ms: i64,
}
