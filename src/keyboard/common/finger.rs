//! The typing fingers — a per-key property assigned by the keyboard's
//! designer + the typist's fingering convention.
//!
//! Pure data. Per-renderer theming (e.g. finger → Color) lives with
//! the renderer, not here.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Finger {
    LPinky,
    LRing,
    LMiddle,
    LIndex,
    LThumb,
    RThumb,
    RIndex,
    RMiddle,
    RRing,
    RPinky,
}
