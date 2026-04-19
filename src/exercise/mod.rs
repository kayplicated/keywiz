//! Exercises — typing-tutor content types driven by the engine.
//!
//! Each exercise owns its content and progression cursor. The
//! engine asks `expected()` for the next target character,
//! compares against input, calls `advance()` on a hit, and
//! queries `fill_display()` to populate exercise-specific fields
//! in the `DisplayState` it hands to the renderer.
//!
//! Exercises never touch stats or the keyboard directly.

pub mod catalog;
pub mod drill;
pub mod text;
pub mod words;

use crate::engine::placement::DisplayState;

pub trait Exercise {
    fn name(&self) -> &str;
    fn short(&self) -> &str;
    fn expected(&self) -> Option<char>;
    fn advance(&mut self);
    fn is_done(&self) -> bool;
    fn fill_display(&self, display: &mut DisplayState);
    /// Handle non-typing control keys (arrow keys in text mode,
    /// for example). Return `true` if handled.
    fn handle_control(&mut self, _key: crossterm::event::KeyEvent) -> bool {
        false
    }
}
