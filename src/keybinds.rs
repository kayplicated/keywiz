//! Global keybinds handled before per-mode input.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::AppContext;
use crate::stats;

pub enum KeybindResult {
    Handled,
    Passthrough,
}

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

fn cycle_keyboard(ctx: &mut AppContext, dir: Dir) {
    let result = match dir {
        Dir::Next => ctx.engine_mut().next_keyboard(),
        Dir::Prev => ctx.engine_mut().prev_keyboard(),
    };
    if result.is_ok() {
        ctx.rebuild_translator();
    }
}

fn cycle_layout(ctx: &mut AppContext, dir: Dir) {
    let result = match dir {
        Dir::Next => ctx.engine_mut().next_layout(),
        Dir::Prev => ctx.engine_mut().prev_layout(),
    };
    let Ok(change) = result else { return };
    stats::persist::save(&change.from, ctx.stats.persistent());
    ctx.stats = stats::StatsTracker::new();
    ctx.stats.set_persistent(stats::persist::load(&change.to));
    ctx.rebuild_translator();
}
