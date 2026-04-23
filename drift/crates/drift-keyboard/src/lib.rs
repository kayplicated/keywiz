//! Keyboard and Layout loaders for the keywiz JSON5 format.

use std::path::Path;

use anyhow::Result;
use drift_core::{Keyboard, Layout};

pub mod keyboard;
pub mod layout;
pub mod writer;

/// Load a keyboard definition from a keywiz JSON5 file.
pub fn load_keyboard(path: &Path) -> Result<Keyboard> {
    keyboard::load(path)
}

/// Load and resolve a layout definition against a keyboard.
pub fn load_layout(path: &Path, keyboard: &Keyboard) -> Result<Layout> {
    layout::load(path, keyboard)
}
