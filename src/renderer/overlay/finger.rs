//! Finger-color overlay — paints each key in the color of the
//! finger that owns it.
//!
//! The original keywiz look. Per-surface config lets users dial
//! down the visual weight while keeping the cue: turn borders off
//! for a labels-only finger hint, or turn labels off for a
//! borders-only frame.
//!
//! Colors are ratatui-native today; a gui renderer would remap the
//! same logical-finger signal to its own palette. The mapping is
//! intentionally here, not on `Finger`, because it's a presentation
//! concern.

use ratatui::style::Color;

use super::{KeyOverlay, KeyPaint};
use crate::engine::placement::Placement;
use crate::keyboard::common::Finger;

/// Which surfaces finger-color should paint. Defaults to label +
/// border (the historical look). Prefs can swap in any combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FingerStyle {
    pub label: bool,
    pub border: bool,
    pub fill: bool,
}

impl Default for FingerStyle {
    fn default() -> Self {
        Self { label: true, border: true, fill: false }
    }
}

impl FingerStyle {
    /// Labels-only — finger cue without the frame weight.
    /// Preset helper for `prefs.json`-driven construction.
    #[allow(dead_code)] // staged for prefs integration
    pub const fn labels_only() -> Self {
        Self { label: true, border: false, fill: false }
    }

    /// Borders-only — frame cue without the label weight.
    #[allow(dead_code)]
    pub const fn borders_only() -> Self {
        Self { label: false, border: true, fill: false }
    }
}

/// Colors each key by the finger that owns it.
#[derive(Debug, Default)]
pub struct FingerOverlay {
    style: FingerStyle,
}

impl FingerOverlay {
    pub fn new(style: FingerStyle) -> Self {
        Self { style }
    }
}

impl KeyOverlay for FingerOverlay {
    fn paint(&self, placement: &Placement) -> KeyPaint {
        let color = finger_color(placement.finger);
        KeyPaint {
            label: self.style.label.then_some(color),
            border: self.style.border.then_some(color),
            fill: self.style.fill.then_some(color),
            modifier: None,
            glyph: None,
        }
    }

    fn name(&self) -> &'static str {
        "finger"
    }
}

fn finger_color(finger: Finger) -> Color {
    match finger {
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
