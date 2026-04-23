//! Reader for the `.dof` layout format used by oxeylyzer.
//!
//! A `.dof` file carries a 3×10 alpha-core layout, a board
//! descriptor (`ortho` / `elora` / `ansi`), and light metadata.
//! This crate parses the document, picks a default keywiz keyboard
//! for the board descriptor, and resolves the character rows into
//! a [`drift_core::Layout`] against a loaded keyboard.
//!
//! The crate is an *adapter*: it owns one format. The keywiz JSON5
//! adapter is [`drift_keyboard`](../drift_keyboard/index.html); a
//! kanata adapter would live as a sibling.

use std::path::Path;

use anyhow::Result;
use drift_core::{Keyboard, Layout};

pub mod board;
pub mod layout;
pub mod parse;

pub use board::default_keyboard_path;
pub use parse::DofDocument;

/// Read a `.dof` file and resolve it into a [`Layout`] against
/// `keyboard`. The keyboard must already be loaded — use
/// [`default_keyboard_path`] to pick a default for the board
/// descriptor, or let the caller override.
pub fn load_layout(path: &Path, keyboard: &Keyboard) -> Result<Layout> {
    let doc = parse::load(path)?;
    layout::resolve(&doc, keyboard)
}
