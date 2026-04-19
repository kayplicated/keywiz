//! Keyboard widget driven by a [`Grid`].
//!
//! Renders whatever buttons the grid declares, positioned relative to
//! home-row center, honoring fractional x/y and per-key width/height so
//! staggered boards, splayed columns, and thumb clusters display
//! accurately. Rotation is stored on the model but the terminal renderer
//! draws axis-aligned — SVG / web renderers can do better.
//!
//! Unmapped buttons (keycodes the active layout doesn't cover) render as
//! dim outlines so the hardware shape stays visible.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::grid::layout::KeyMapping;
use crate::grid::{Grid, GridButton};
use crate::physical::human_name;
use crate::stats::Stats;
use crate::ui::heatmap;

/// Terminal cells per unit of grid x (one key-width in columns).
pub const CELL_W: u16 = 5;
/// Terminal lines per unit of grid y (one row-height in lines).
///
/// The terminal renderer is intentionally **schematic**: it respects
/// `x` (so row-stagger boards like ANSI render with their real
/// horizontal shifts) but snaps vertical positions to the key's
/// nominal `row`, flattening column-stagger splay. Fractional y lives
/// on in the data for richer renderers (future SVG/desktop/web) but
/// terminal pretends the keyboard is row-wise flat.
pub const CELL_H: u16 = 3;

/// Terminal height in lines needed to render `grid` at its natural size.
/// Lets the mode layouts allocate a keyboard area that actually fits
/// (staggered boards with splay can be significantly taller than a plain
/// ortho 3-row layout).
pub fn grid_height(grid: &Grid) -> u16 {
    if grid.buttons.is_empty() {
        return 0;
    }
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for b in &grid.buttons {
        let half_h = b.height / 2.0;
        min_y = min_y.min(b.y - half_h);
        max_y = max_y.max(b.y + half_h);
    }
    ((max_y - min_y) * CELL_H as f32).ceil() as u16
}

/// Render a [`Grid`]. `highlight` optionally highlights the button whose
/// layout mapping produces that character. `heat` enables the heatmap
/// colorization (finger colors otherwise).
pub fn render_grid(
    f: &mut Frame,
    area: Rect,
    grid: &Grid,
    highlight: Option<char>,
    heat: Option<&Stats>,
) {
    if grid.buttons.is_empty() {
        return;
    }

    let (min_x, max_x, min_y, max_y) = grid_extent(grid);
    let width_units = max_x - min_x;
    let height_units = max_y - min_y;
    let widget_w = (width_units * CELL_W as f32).ceil() as u16;
    let widget_h = (height_units * CELL_H as f32).ceil() as u16;

    let origin_x = area.x + area.width.saturating_sub(widget_w) / 2;
    let origin_y = area.y + area.height.saturating_sub(widget_h) / 2;

    let highlight_lower = highlight.map(|c| c.to_ascii_lowercase());

    for btn in &grid.buttons {
        // Upper-left corner of the button's bounding box in grid units.
        let btn_left = btn.x - btn.width / 2.0;
        let btn_top = btn.y - btn.height / 2.0;

        let col = ((btn_left - min_x) * CELL_W as f32).round() as u16;
        let row = ((btn_top - min_y) * CELL_H as f32).round() as u16;
        let w = ((btn.width * CELL_W as f32).round() as u16).max(3);
        let h = ((btn.height * CELL_H as f32).round() as u16).max(3);

        let x = origin_x + col;
        let y = origin_y + row;

        let is_highlighted = match (&btn.mapping, highlight_lower) {
            (Some(KeyMapping::Char { lower, .. }), Some(h)) => *lower == h,
            _ => false,
        };

        render_button(f, x, y, w, h, area, btn, is_highlighted, heat);
    }
}

/// Bounding box of the whole grid in grid units, accounting for each
/// button's own width/height so wide keys extend the bounds properly.
fn grid_extent(grid: &Grid) -> (f32, f32, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for b in &grid.buttons {
        let half_w = b.width / 2.0;
        let half_h = b.height / 2.0;
        min_x = min_x.min(b.x - half_w);
        max_x = max_x.max(b.x + half_w);
        min_y = min_y.min(b.y - half_h);
        max_y = max_y.max(b.y + half_h);
    }
    (min_x, max_x, min_y, max_y)
}

fn render_button(
    f: &mut Frame,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    area: Rect,
    btn: &GridButton,
    is_highlighted: bool,
    heat: Option<&Stats>,
) {
    if x + w > area.x + area.width || y + h > area.y + area.height {
        return;
    }
    let cell = Rect::new(x, y, w, h);

    let Some(mapping) = &btn.mapping else {
        // Unmapped key: dim outline, no character.
        let style = Style::default().fg(Color::DarkGray);
        f.render_widget(Paragraph::new(box_lines(w, h, "", style)), cell);
        return;
    };

    let (label, lower_char) = match mapping {
        KeyMapping::Char { lower, .. } => (lower.to_string(), Some(*lower)),
        KeyMapping::Named { name } => (human_name(&format!("mods_{name}")), None),
    };

    // Heatmap is keyed by typed character. Named keys (Shift/Tab/…)
    // don't participate in heat — show them in finger color either way.
    let color = match (heat, lower_char) {
        (Some(stats), Some(ch)) => heatmap::heat_color(stats, ch).unwrap_or(Color::DarkGray),
        _ => btn.finger.color(),
    };

    if is_highlighted {
        let border = Style::default().fg(color).bold();
        let letter = Style::default().fg(Color::White).bold();
        f.render_widget(
            Paragraph::new(box_lines_highlighted(w, h, &label, border, letter)),
            cell,
        );
    } else {
        let style = Style::default().fg(color);
        f.render_widget(Paragraph::new(box_lines(w, h, &label, style)), cell);
    }
}

/// Draw a single-line border box of `w` columns by `h` lines, centering
/// `label` on the middle row. `label` may be any short string (single
/// character for typing keys, short name like "Shift" or "Tab" for
/// named keys, empty for unmapped keys).
fn box_lines(w: u16, h: u16, label: &str, style: Style) -> Vec<Line<'static>> {
    if w < 3 || h < 3 {
        return vec![Line::raw("")];
    }
    let inner_w = (w - 2) as usize;
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(h as usize);

    lines.push(Line::from(Span::styled(
        format!("┌{}┐", "─".repeat(inner_w)),
        style,
    )));

    let middle_idx = (h - 2) / 2;
    let label = truncate_to(label, inner_w);
    for i in 0..(h - 2) {
        if i == middle_idx {
            let label_w = label.chars().count();
            let left_pad = inner_w.saturating_sub(label_w) / 2;
            let right_pad = inner_w.saturating_sub(label_w) - left_pad;
            let content = format!(
                "│{}{}{}│",
                " ".repeat(left_pad),
                label,
                " ".repeat(right_pad)
            );
            lines.push(Line::from(Span::styled(content, style)));
        } else {
            lines.push(Line::from(Span::styled(
                format!("│{}│", " ".repeat(inner_w)),
                style,
            )));
        }
    }

    lines.push(Line::from(Span::styled(
        format!("└{}┘", "─".repeat(inner_w)),
        style,
    )));
    lines
}

/// Highlighted variant: double-line border and bold white label.
fn box_lines_highlighted(
    w: u16,
    h: u16,
    label: &str,
    border: Style,
    letter: Style,
) -> Vec<Line<'static>> {
    if w < 3 || h < 3 {
        return vec![Line::raw("")];
    }
    let inner_w = (w - 2) as usize;
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(h as usize);

    lines.push(Line::from(Span::styled(
        format!("╔{}╗", "═".repeat(inner_w)),
        border,
    )));

    let middle_idx = (h - 2) / 2;
    let label = truncate_to(label, inner_w);
    for i in 0..(h - 2) {
        if i == middle_idx {
            let label_w = label.chars().count();
            let left_pad = inner_w.saturating_sub(label_w) / 2;
            let right_pad = inner_w.saturating_sub(label_w) - left_pad;
            lines.push(Line::from(vec![
                Span::styled("║", border),
                Span::styled(" ".repeat(left_pad), letter),
                Span::styled(label.clone(), letter),
                Span::styled(" ".repeat(right_pad), letter),
                Span::styled("║", border),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                format!("║{}║", " ".repeat(inner_w)),
                border,
            )));
        }
    }

    lines.push(Line::from(Span::styled(
        format!("╚{}╝", "═".repeat(inner_w)),
        border,
    )));
    lines
}

/// Truncate `s` to at most `max` characters (not bytes). Used to fit
/// named-key labels like "Shift" inside a narrow key-cap.
fn truncate_to(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        chars.into_iter().take(max).collect()
    }
}
