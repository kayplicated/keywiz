//! A resolved layout mapping characters to physical keys.
//!
//! Only alpha-producing bindings are retained. Named keys (shift,
//! tab, etc.) are filtered out at load time.

use std::collections::HashMap;

use crate::Key;

/// A layout: char → physical key. Uppercase chars collapse to
/// lowercase before lookup.
#[derive(Debug, Clone)]
pub struct Layout {
    pub name: String,
    pub positions: HashMap<char, Key>,
}

impl Layout {
    /// Position of a character on this layout, if bound. Lowercases
    /// the lookup so callers don't have to.
    pub fn position(&self, ch: char) -> Option<&Key> {
        self.positions.get(&ch.to_ascii_lowercase())
    }
}
