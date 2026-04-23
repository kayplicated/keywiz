//! The empty overlay — renderer baseline, nothing painted.

use super::{KeyOverlay, KeyPaint};
use crate::engine::placement::Placement;

/// Paints nothing. Renderer falls back to its built-in defaults
/// for every key. This is keywiz's default look — plain monochrome
/// keys, overlays must be turned on.
#[derive(Debug, Default)]
pub struct NoneOverlay;

impl KeyOverlay for NoneOverlay {
    fn paint(&self, _placement: &Placement) -> KeyPaint {
        KeyPaint::none()
    }

    fn name(&self) -> &'static str {
        "none"
    }
}
