//! Minimal `LayoutSnapshot` / `KeyboardSnapshot` builders so tests
//! don't have to reach for real drift/keywiz types to exercise the
//! store.

use keywiz_stats::{KeyboardHash, KeyboardSnapshot, LayoutHash, LayoutSnapshot};

pub fn layout(name: &str, hash_hex: &str) -> LayoutSnapshot {
    LayoutSnapshot {
        hash: LayoutHash(hash_hex.to_string()),
        name: name.to_string(),
        canonical_json: format!("{{\"name\":\"{name}\"}}"),
        first_seen_ms: 0,
    }
}

pub fn keyboard(name: &str, hash_hex: &str) -> KeyboardSnapshot {
    KeyboardSnapshot {
        hash: KeyboardHash(hash_hex.to_string()),
        name: name.to_string(),
        canonical_json: format!("{{\"name\":\"{name}\"}}"),
        first_seen_ms: 0,
    }
}
