//! A physical keyboard — a named collection of [`PhysicalKey`]s.

use crate::physical::engine::{Bounds, Point};
use crate::physical::keys::PhysicalKey;

/// A complete physical keyboard.
#[derive(Debug, Clone)]
pub struct PhysicalKeyboard {
    pub name: String,
    pub short: String,
    pub description: String,
    pub keys: Vec<PhysicalKey>,
}

impl PhysicalKeyboard {
    /// Bounding box of all keys (using key centers — ignores width/height
    /// for now; refine when renderers need edge-accurate bounds).
    pub fn bounds(&self) -> Bounds {
        Bounds::enclosing(self.keys.iter().map(|k| Point::new(k.x, k.y)))
    }

    /// Find a key by id. `None` if no key with that id exists.
    pub fn get(&self, id: &str) -> Option<&PhysicalKey> {
        self.keys.iter().find(|k| k.id == id)
    }
}
