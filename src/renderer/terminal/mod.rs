//! Terminal renderer for the active keyboard + layout.
//!
//! Reads keyboards through `trait Keyboard`, dispatches per-block
//! placement by stagger type, draws ratatui widgets. The renderer
//! is stateless — each call to `render_keyboard` rebuilds
//! placements from the current engine data.
//!
//! Also owns terminal-specific UI helpers (`centered_content_layout`,
//! footer rendering) that used to live in `src/ui/`.

pub mod draw;
pub mod heatmap;
pub mod layout;
pub mod naming;
pub mod placement;

pub use layout::{centered_content_layout, render_footer, ContentAreas};
pub use placement::{CELL_H, CELL_W};

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::keyboard::{Block, Keyboard};
use crate::mapping::{KeyMapping, Layout};
use crate::stats::Stats;

use placement::{place_block, TerminalPlacement};

/// Height in terminal lines needed to render this keyboard naturally.
/// Mode layouts use this to size the keyboard area; halcyon's tall
/// main block + thumb cluster wants more vertical space than a bare
/// ortho 3-row board.
pub fn keyboard_height(keyboard: &dyn Keyboard) -> u16 {
    let mut min_row = i32::MAX;
    let mut max_row = i32::MIN;
    let mut max_height = 1;
    for block in keyboard.blocks() {
        for placement in place_block(block) {
            min_row = min_row.min(placement.row);
            max_row = max_row.max(placement.row + placement.height - 1);
            max_height = max_height.max(placement.height);
        }
    }
    if min_row > max_row {
        return 0;
    }
    (max_row - min_row + 1) as u16
}

/// Render the active keyboard. `highlight` optionally bolds the key
/// whose layout mapping produces that character (used for drill
/// mode's "press this next" indicator). `heat` enables heatmap
/// coloring when provided.
pub fn render_keyboard(
    f: &mut Frame,
    area: Rect,
    keyboard: &dyn Keyboard,
    layout: &Layout,
    highlight: Option<char>,
    heat: Option<&Stats>,
) {
    if area.width < 3 || area.height < 3 {
        return;
    }

    // Gather all placements, compute the extent, center inside area.
    let mut all: Vec<TerminalPlacement> = Vec::new();
    for block in keyboard.blocks() {
        all.extend(place_block(block));
    }
    if all.is_empty() {
        return;
    }

    let mut min_col = i32::MAX;
    let mut max_col = i32::MIN;
    let mut min_row = i32::MAX;
    let mut max_row = i32::MIN;
    for p in &all {
        min_col = min_col.min(p.col);
        max_col = max_col.max(p.col + p.width - 1);
        min_row = min_row.min(p.row);
        max_row = max_row.max(p.row + p.height - 1);
    }
    let widget_w = (max_col - min_col + 1).max(0) as u16;
    let widget_h = (max_row - min_row + 1).max(0) as u16;

    let origin_x = area.x + area.width.saturating_sub(widget_w) / 2;
    let origin_y = area.y + area.height.saturating_sub(widget_h) / 2;

    let highlight_lower = highlight.map(|c| c.to_ascii_lowercase());

    for placement in all {
        let x = origin_x as i32 + placement.col - min_col;
        let y = origin_y as i32 + placement.row - min_row;
        if x < 0 || y < 0 {
            continue;
        }
        let x = x as u16;
        let y = y as u16;
        let w = placement.width as u16;
        let h = placement.height as u16;
        if x + w > area.x + area.width || y + h > area.y + area.height {
            continue;
        }
        let rect = Rect::new(x, y, w, h);

        let mapping = layout.get(&placement.key.id);
        let is_highlighted = match (mapping, highlight_lower) {
            (Some(KeyMapping::Char { lower, .. }), Some(h)) => *lower == h,
            _ => false,
        };

        draw::draw_key(f, rect, &placement, mapping, is_highlighted, heat);
    }
    // Suppress unused-import lints from modules referenced only through
    // public re-exports.
    let _ = <dyn Block>::cluster;
}
