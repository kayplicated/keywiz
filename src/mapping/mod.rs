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

#[derive(Debug, Clone)]
pub struct Layout {
    pub short: String,
    pub mappings: HashMap<String, KeyMapping>,
}

impl Layout {
    pub fn get(&self, id: &str) -> Option<&KeyMapping> {
        self.mappings.get(id)
    }
}
