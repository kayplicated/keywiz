//! Data-driven keyboard + layout composition.
//!
//! A **keyboard** is a physical button grid: every key declared once with a
//! stable evdev keycode, a home-row-centered screen position, and a finger
//! assignment. Keyboards live as JSON files under `keyboards/`.
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
}
