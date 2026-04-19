//! Global keybinds handled before per-mode input.
//!
//! Lives here so the event loop stays small and adding a new global
//! shortcut is one edit in one file. Each keybind either mutates
//! [`AppContext`] directly or routes through [`GridManager`] — no
//! mode-specific logic.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::AppContext;
use crate::stats;

/// What the event loop should do after [`handle_shared`] runs.
pub enum KeybindResult {
    /// Keybind handled; skip mode dispatch for this key.
    Handled,
    /// No global binding matched; forward to the active mode.
    Passthrough,
}

/// Apply any global keybind triggered by `key`. Returns [`KeybindResult`]
/// so the caller knows whether to forward the event to the active mode.
pub fn handle_shared(key: KeyEvent, ctx: &mut AppContext) -> KeybindResult {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Tab => {
            ctx.show_keyboard = !ctx.show_keyboard;
            KeybindResult::Handled
        }
        KeyCode::F(2) => {
            ctx.show_heatmap = !ctx.show_heatmap;
            KeybindResult::Handled
        }
        KeyCode::Up if ctrl => {
            cycle_keyboard(ctx, Dir::Prev);
            KeybindResult::Handled
        }
        KeyCode::Down if ctrl => {
            cycle_keyboard(ctx, Dir::Next);
            KeybindResult::Handled
        }
        KeyCode::Left if ctrl => {
            cycle_layout(ctx, Dir::Prev);
            KeybindResult::Handled
        }
        KeyCode::Right if ctrl => {
            cycle_layout(ctx, Dir::Next);
            KeybindResult::Handled
        }
        _ => KeybindResult::Passthrough,
    }
}

enum Dir {
    Next,
    Prev,
}

/// Cycle to the next/previous keyboard. Single-grid managers (the
/// kanata path) hold one keyboard, so cycling is a no-op there.
fn cycle_keyboard(ctx: &mut AppContext, dir: Dir) {
    let _ = match dir {
        Dir::Next => ctx.grid_manager.next_keyboard(),
        Dir::Prev => ctx.grid_manager.prev_keyboard(),
    };
}

/// Cycle to the next/previous layout. Layout changes swap the per-layout
/// persistent stats: save the outgoing, load the incoming.
fn cycle_layout(ctx: &mut AppContext, dir: Dir) {
    let change = match dir {
        Dir::Next => ctx.grid_manager.next_layout(),
        Dir::Prev => ctx.grid_manager.prev_layout(),
    };
    let Ok(change) = change else { return };
    stats::persist::save(&change.from, ctx.stats.persistent());
    ctx.stats = stats::StatsTracker::new();
    ctx.stats
        .set_persistent(stats::persist::load(&change.to));
}
