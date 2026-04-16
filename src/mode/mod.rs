//! Training modes. Each mode provides content, flow, and wires together
//! engines and UI components. Modes are self-contained — adding a new mode
//! should not require changes to the event loop or other modes.

pub mod drill;
pub mod select;
pub mod text;
pub mod words;

use crate::app::AppContext;
use crossterm::event::KeyEvent;
use ratatui::Frame;

/// What the event loop should do after a mode handles input.
pub enum ModeResult {
    /// Stay in the current mode.
    Stay,
    /// Quit the application.
    Quit,
    /// Switch to a different mode.
    SwitchTo(ActiveMode),
}

/// The currently active mode. Owns all mode-specific state.
pub enum ActiveMode {
    Select(select::SelectMode),
    Drill(drill::DrillMode),
    Words(words::WordsMode),
    Text(text::TextMode),
}

impl ActiveMode {
    /// Handle a key event. Shared keys (Tab, BackTab) are handled by the
    /// event loop before this is called.
    pub fn handle_input(&mut self, key: KeyEvent, ctx: &mut AppContext) -> ModeResult {
        match self {
            ActiveMode::Select(m) => m.handle_input(key, ctx),
            ActiveMode::Drill(m) => m.handle_input(key, ctx),
            ActiveMode::Words(m) => m.handle_input(key, ctx),
            ActiveMode::Text(m) => m.handle_input(key, ctx),
        }
    }

    /// Render the current mode.
    pub fn render(&self, f: &mut Frame, ctx: &AppContext) {
        match self {
            ActiveMode::Select(m) => m.render(f, ctx),
            ActiveMode::Drill(m) => m.render(f, ctx),
            ActiveMode::Words(m) => m.render(f, ctx),
            ActiveMode::Text(m) => m.render(f, ctx),
        }
    }
}
