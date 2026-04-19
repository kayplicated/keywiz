//! Per-target renderers.
//!
//! Each renderer consumes the same keyboard + layout through the
//! `trait Keyboard` / `Layout` interfaces and produces output for
//! its target. Terminal is live; desktop and webui are stubs for
//! later.

pub mod terminal;
pub mod desktop;
pub mod webui;
