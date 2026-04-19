//! Character layouts — map physical key ids to what the key does when
//! pressed.
//!
//! A [`Layout`] is a partial function from physical id to behavior.
//! Keys not covered by the layout are dead — they render blank and
//! produce no character. This is a feature, not an error: a 36-key
//! layout applied to a 60-key board leaves 24 keys dead; a layout that
//! names positions the board doesn't have simply has unused entries.
//!
//! Identity lives in the physical key's id; layouts never address keys
//! by geometry.

use std::collections::HashMap;

/// What a physical key does when pressed.
///
/// `Char` covers typing keys (letters, digits, punctuation — anything
/// that produces a character, including Space). `Named` covers
/// non-character actions (Shift, Tab, Ctrl, Enter, Escape — keys that
/// trigger behavior the terminal doesn't see as a character).
#[derive(Debug, Clone)]
pub enum KeyMapping {
    /// A typed character with its shifted variant.
    Char { lower: char, upper: char },
    /// A non-character action. The name is free-form; consumers may
    /// interpret it (e.g. `"shift"`, `"tab"`, `"enter"`).
    Named { name: String },
}

impl KeyMapping {
    /// The unshifted character produced, if this is a [`Char`] mapping.
    pub fn lower_char(&self) -> Option<char> {
        if let KeyMapping::Char { lower, .. } = self {
            Some(*lower)
        } else {
            None
        }
    }
}

/// A layout: a lookup from physical id to mapping.
#[derive(Debug, Clone)]
pub struct Layout {
    pub name: String,
    pub short: String,
    pub mappings: HashMap<String, KeyMapping>,
}

impl Layout {
    pub fn get(&self, id: &str) -> Option<&KeyMapping> {
        self.mappings.get(id)
    }
}
