//! The typing fingers.
//!
//! A finger assignment is a property of a physical key on a specific
//! keyboard — decided by the board's designer and the typist's fingering
//! convention. Lives in the engine module because it's part of the
//! coordinate/layout vocabulary, not an attribute of any particular key.

use ratatui::style::Color;
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

impl Finger {
    /// Terminal color used to tint keys assigned to this finger.
    pub fn color(self) -> Color {
        match self {
            Finger::LPinky => Color::Red,
            Finger::LRing => Color::Yellow,
            Finger::LMiddle => Color::Green,
            Finger::LIndex => Color::Cyan,
            Finger::LThumb => Color::DarkGray,
            Finger::RThumb => Color::DarkGray,
            Finger::RIndex => Color::Blue,
            Finger::RMiddle => Color::Magenta,
            Finger::RRing => Color::Yellow,
            Finger::RPinky => Color::Red,
        }
    }
}
