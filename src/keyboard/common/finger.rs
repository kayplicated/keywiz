//! The typing fingers — a per-key property assigned by the keyboard's
//! designer + the typist's fingering convention.

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
