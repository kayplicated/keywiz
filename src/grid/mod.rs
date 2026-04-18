//! Data-driven keyboard + layout composition.
//!
//! A **keyboard** is a physical button grid: every key declared once with a
//! stable evdev keycode, a home-row-centered screen position, and a finger
//! assignment. Keyboards live as JSON files under `keyboards/`.
//!
//! # Coordinate convention
//!
//! Positions are in key-width units relative to home-row center. Rows sit
//! one unit apart; columns one unit apart; column-stagger offsets use
//! half-unit y values. By convention:
//! - home row: `y = 0`
//! - top row: `y = -1`
//! - bottom row: `y = 1`
//! - number row: `y = -2`
//!
//! Drill mode uses these y-values to derive character sets for each
//! level, so custom keyboards should follow the convention.
//!
//! A **layout** maps keycodes to lowercase + shifted characters. Layouts
//! live under `layouts/` as JSON. A generic layout (e.g. `qwerty.json`)
//! works on any keyboard; a hardware-specific override can be shipped as
//! `{layout}-{keyboard}.json` and takes priority when resolved against that
//! keyboard.
//!
//! A [`Grid`] is a keyboard composed with a layout — one [`GridButton`] per
//! physical key, with the character mapping resolved in. Buttons the layout
//! doesn't cover still appear in the grid but carry no character; the
//! widget renders them dimmed so the hardware shape stays honest.
//!
//! [`GridManager`] owns the catalog of keyboards and layouts, tracks what's
//! active, and exposes setters + cycling methods that a future keybind
//! layer can call directly (no mode-specific logic involved).

pub mod keyboard;
pub mod layout;
pub mod manager;

pub use keyboard::{Keyboard, KeyboardButton};
pub use layout::{KeyMapping, Layout};
pub use manager::{GridManager, LayoutChange};

use crate::layout::Finger;

/// A physical key composed with its (optional) character mapping.
#[derive(Debug, Clone)]
pub struct GridButton {
    pub code: String,
    pub x: f32,
    pub y: f32,
    pub finger: Finger,
    /// `None` when the active layout doesn't map this keycode. The widget
    /// still draws the button as an empty, dimmed outline.
    pub mapping: Option<KeyMapping>,
}

/// The active keyboard + layout as a flat list of drawable buttons.
///
/// Consumers (the widget, input handling) don't need to know whether a
/// button came from the generic layout, a hardware override, or a kanata
/// file — they just see a grid.
#[derive(Debug, Clone)]
pub struct Grid {
    pub keyboard_name: String,
    pub keyboard_short: String,
    pub layout_name: String,
    pub layout_short: String,
    pub buttons: Vec<GridButton>,
}

impl Grid {
    /// Compose a keyboard with a layout. Each physical button is paired
    /// with its character mapping if the layout covers that keycode.
    pub fn compose(keyboard: &Keyboard, layout: &Layout) -> Self {
        let buttons = keyboard
            .buttons
            .iter()
            .map(|btn| GridButton {
                code: btn.code.clone(),
                x: btn.x,
                y: btn.y,
                finger: btn.finger,
                mapping: layout.keys.get(&btn.code).cloned(),
            })
            .collect();
        Grid {
            keyboard_name: keyboard.name.clone(),
            keyboard_short: keyboard.short.clone(),
            layout_name: layout.name.clone(),
            layout_short: layout.short.clone(),
            buttons,
        }
    }

    /// Find a button by the character it produces (unshifted).
    pub fn find_by_char(&self, ch: char) -> Option<&GridButton> {
        let target = ch.to_ascii_lowercase();
        self.buttons.iter().find(|b| {
            b.mapping
                .as_ref()
                .map(|m| m.lower == target)
                .unwrap_or(false)
        })
    }

    /// All characters produced by buttons at `y` (approximately — within
    /// half a row unit). Only alphabetic characters are returned, so drill
    /// modes can target letters without worrying about punctuation rows.
    pub fn alpha_chars_at_y(&self, y: f32) -> Vec<char> {
        self.buttons
            .iter()
            .filter(|b| (b.y - y).abs() < 0.5)
            .filter_map(|b| b.mapping.as_ref().map(|m| m.lower))
            .filter(|c| c.is_alphabetic())
            .collect()
    }

    /// Alphabetic characters on the home row.
    ///
    /// Assumes the keyboard convention that home row sits at `y = 0` —
    /// every shipped keyboard follows this, and custom keyboards should
    /// too so drill levels remain consistent.
    pub fn home_row_chars(&self) -> Vec<char> {
        self.alpha_chars_at_y(0.0)
    }

    /// Alphabetic characters from home row plus the row above it (top row).
    /// Assumes home at `y = 0`, top row at `y = -1`.
    pub fn home_and_top_chars(&self) -> Vec<char> {
        let mut chars = self.alpha_chars_at_y(0.0);
        chars.extend(self.alpha_chars_at_y(-1.0));
        chars
    }

    /// All alphabetic characters produced by any button on the grid.
    pub fn all_alpha_chars(&self) -> Vec<char> {
        self.buttons
            .iter()
            .filter_map(|b| b.mapping.as_ref().map(|m| m.lower))
            .filter(|c| c.is_alphabetic())
            .collect()
    }
}
