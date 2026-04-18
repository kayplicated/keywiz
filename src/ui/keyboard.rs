use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::layout::{Key, Layout};
use crate::stats::Stats;
use crate::ui::heatmap;

/// Key cell width in characters (including borders).
const KEY_W: u16 = 5;
/// Key cell height in lines.
const KEY_H: u16 = 3;

/// Row offsets (in half-key units) to simulate stagger on standard keyboards.
const ROW_OFFSETS: [u16; 4] = [0, 1, 3, 5];

/// Column stagger offsets for split columnar keyboards (in lines, applied vertically).
/// Pinky: +1, Ring: 0, Middle: -1, Index: 0, Inner index: +1
/// Applied per-column: [0, 1, 2, 3, 4, 5] on each half
#[allow(dead_code)]
const COL_STAGGER: [i16; 6] = [1, 0, -1, 0, 1, 1];

/// Split gap width in characters.
const SPLIT_GAP: u16 = 4;

/// Render the keyboard widget.
///
/// When `heat` is `Some`, each key is colored by per-key accuracy from the
/// given stats (gray for keys without enough samples) instead of by finger.
pub fn render_keyboard(
    f: &mut Frame,
    area: Rect,
    layout: &Layout,
    highlight: Option<char>,
    split: bool,
    heat: Option<&Stats>,
) {
    if split {
        render_split(f, area, layout, highlight, heat);
    } else {
        render_standard(f, area, layout, highlight, heat);
    }
}

/// Color to use for a key's border. Heatmap overrides finger color when
/// provided; keys without enough heatmap samples fall back to dark gray.
fn key_color(key: &Key, heat: Option<&Stats>) -> Color {
    match heat {
        Some(stats) => heatmap::heat_color(stats, key.lower).unwrap_or(Color::DarkGray),
        None => key.finger.color(),
    }
}

fn render_key(
    f: &mut Frame,
    x: u16,
    y: u16,
    area: Rect,
    key: &Key,
    is_highlighted: bool,
    heat: Option<&Stats>,
) {
    if x + KEY_W > area.x + area.width || y + KEY_H > area.y + area.height {
        return;
    }
    let key_area = Rect::new(x, y, KEY_W, KEY_H);
    let color = key_color(key, heat);

    let lines = if is_highlighted {
        let border = Style::default().fg(color).bold();
        let letter = Style::default().fg(Color::White).bold();
        vec![
            Line::from(Span::styled("╔═══╗", border)),
            Line::from(vec![
                Span::styled("║ ", border),
                Span::styled(format!("{}", key.lower), letter),
                Span::styled(" ║", border),
            ]),
            Line::from(Span::styled("╚═══╝", border)),
        ]
    } else {
        let style = Style::default().fg(color);
        vec![
            Line::from(Span::styled("┌───┐", style)),
            Line::from(Span::styled(format!("│ {} │", key.lower), style)),
            Line::from(Span::styled("└───┘", style)),
        ]
    };

    f.render_widget(Paragraph::new(lines), key_area);
}

fn render_standard(
    f: &mut Frame,
    area: Rect,
    layout: &Layout,
    highlight: Option<char>,
    heat: Option<&Stats>,
) {
    let highlight_lower = highlight.map(|c| c.to_ascii_lowercase());

    let kb_width = layout.rows.iter().enumerate()
        .map(|(i, r)| ROW_OFFSETS[i] + (r.keys.len() as u16) * KEY_W)
        .max()
        .unwrap_or(65);
    let kb_height = 4 * KEY_H;

    let x_offset = area.x + area.width.saturating_sub(kb_width) / 2;
    let y_offset = area.y + area.height.saturating_sub(kb_height) / 2;

    for (row_idx, row) in layout.rows.iter().enumerate() {
        let stagger = ROW_OFFSETS[row_idx];
        let row_y = y_offset + (row_idx as u16) * KEY_H;

        for (col_idx, key) in row.keys.iter().enumerate() {
            let key_x = x_offset + stagger + (col_idx as u16) * KEY_W;
            let is_highlighted = highlight_lower == Some(key.lower);
            render_key(f, key_x, row_y, area, key, is_highlighted, heat);
        }
    }
}

fn render_split(
    f: &mut Frame,
    area: Rect,
    layout: &Layout,
    highlight: Option<char>,
    heat: Option<&Stats>,
) {
    let highlight_lower = highlight.map(|c| c.to_ascii_lowercase());

    // Split: number row at 6 (` 1 2 3 4 5 | 6 7 8 9 0 - =), alpha rows at 5
    let split_at = |row_idx: usize, len: usize| -> usize {
        if row_idx == 0 { 6.min(len) } else { 5.min(len) }
    };

    let max_left: u16 = 5;  // alpha rows have 5 on left
    let max_right: u16 = 5; // alpha rows have ~5 on right after filtering
    let kb_width = max_left * KEY_W + SPLIT_GAP + max_right * KEY_W;
    let kb_height = 4 * KEY_H;

    let x_offset = area.x + area.width.saturating_sub(kb_width) / 2;
    let y_offset = area.y + area.height.saturating_sub(kb_height) / 2;

    // The split edge is at x_offset + max_left * KEY_W
    let split_edge = x_offset + max_left * KEY_W;

    // Keys to exclude in split mode (brackets, backslash, grave, dash, equals)
    let exclude = ['`', '-', '=', '[', ']', '\\'];

    for (row_idx, row) in layout.rows.iter().enumerate() {
        let keys = &row.keys;
        let sp = split_at(row_idx, keys.len());
        let row_y = y_offset + (row_idx as u16) * KEY_H;

        // Left half — filter, then right-align to split edge
        let left_keys: Vec<&Key> = keys[..sp].iter()
            .filter(|k| !exclude.contains(&k.lower))
            .collect();
        let left_width = (left_keys.len() as u16) * KEY_W;
        let left_start = split_edge - left_width;
        for (col_idx, key) in left_keys.iter().enumerate() {
            let key_x = left_start + (col_idx as u16) * KEY_W;
            let is_highlighted = highlight_lower == Some(key.lower);
            render_key(f, key_x, row_y, area, key, is_highlighted, heat);
        }

        // Right half — filter, then left-align from split edge + gap
        let right_keys: Vec<&Key> = keys[sp..].iter()
            .filter(|k| !exclude.contains(&k.lower))
            .collect();
        let right_start = split_edge + SPLIT_GAP;
        for (col_idx, key) in right_keys.iter().enumerate() {
            let key_x = right_start + (col_idx as u16) * KEY_W;
            let is_highlighted = highlight_lower == Some(key.lower);
            render_key(f, key_x, row_y, area, key, is_highlighted, heat);
        }
    }
}
