//! A keyboard's alpha-core geometry.

use std::collections::HashMap;

use crate::{Key, KeyId};

/// All scorable keys on a keyboard, keyed by id.
///
/// Non-alpha keys (thumbs, outer columns, number row) may or may not
/// be present depending on what the loader filters. Analyzers should
/// check `Row::is_alpha` rather than assume all keys in the map are
/// alpha-core.
#[derive(Debug, Clone)]
pub struct Keyboard {
    pub name: String,
    pub keys: HashMap<KeyId, Key>,
}

impl Keyboard {
    /// Look up a key by id.
    pub fn key(&self, id: &KeyId) -> Option<&Key> {
        self.keys.get(id)
    }

    /// Look up a key by raw string id. Convenience for callers that
    /// haven't converted to `KeyId` yet.
    pub fn key_by_str(&self, id: &str) -> Option<&Key> {
        self.keys.get(&KeyId(id.to_string()))
    }
}
