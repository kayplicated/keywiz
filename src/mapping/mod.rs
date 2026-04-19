//! Layouts — partial function from physical key id to behavior.
//!
//! A layout maps physical key ids to `KeyMapping::Char` (typed
//! character) or `KeyMapping::Named` (modifier / action). Ids not in
//! the layout's domain render as dead — not an error, just "this
//! key does nothing under this layout."
//!
//! Layouts know nothing about geometry. They're pure lookup tables.
//! The keyboard's id scheme and the layout's id keys must agree;
//! that's the social contract covered by the naming convention in
//! `docs/physical-model.md`.

pub mod loader;

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum KeyMapping {
    /// A typed character with its shifted variant.
    Char { lower: char, upper: char },
    /// A non-character action. Free-form name; consumers interpret
    /// (e.g. `"shift"`, `"tab"`, `"enter"`).
    Named { name: String },
}

impl KeyMapping {
    pub fn lower_char(&self) -> Option<char> {
        if let KeyMapping::Char { lower, .. } = self {
            Some(*lower)
        } else {
            None
        }
    }
}

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

    /// Reverse lookup: given a typed character, find the id that
    /// would produce it. Used by the engine to translate terminal
    /// input back to physical ids. Shifted characters are resolved
    /// too. Returns the first match; ambiguous layouts (same char
    /// assigned to multiple ids) are their author's problem.
    pub fn id_for_char(&self, ch: char) -> Option<&str> {
        self.mappings.iter().find_map(|(id, m)| match m {
            KeyMapping::Char { lower, upper } if *lower == ch || *upper == ch => {
                Some(id.as_str())
            }
            _ => None,
        })
    }
}
