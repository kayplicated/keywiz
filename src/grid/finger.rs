//! Finger assignments and their associated terminal colors.
//!
//! Used by both the keyboard widget (to color each key by which finger
//! types it) and the heatmap (as the fallback color when a key has no
//! accumulated heat).

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Finger {
    LPinky,
    LRing,
    LMiddle,
    LIndex,
    RIndex,
    RMiddle,
    RRing,
    RPinky,
}

impl Finger {
    pub fn color(self) -> Color {
        match self {
            Finger::LPinky => Color::Red,
            Finger::LRing => Color::Yellow,
            Finger::LMiddle => Color::Green,
            Finger::LIndex => Color::Cyan,
            Finger::RIndex => Color::Blue,
            Finger::RMiddle => Color::Magenta,
            Finger::RRing => Color::Yellow,
            Finger::RPinky => Color::Red,
        }
    }
}
