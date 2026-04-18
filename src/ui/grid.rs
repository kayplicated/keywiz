//! Keyboard widget driven by a [`Grid`].
//!
//! Unlike the legacy `keyboard` widget, this one doesn't assume US
//! geometry — it renders whatever buttons the grid declares, positioned
//! relative to home-row center. Unmapped buttons (keycodes the active
//! layout doesn't cover) render as dim outlines so the hardware shape
//! stays visible.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::grid::{Grid, GridButton};
use crate::stats::Stats;
use crate::ui::heatmap;

/// Terminal cells per unit of grid x (one key-width).
const CELL_W: u16 = 5;
/// Terminal lines per unit of grid y (one row-height).
const CELL_H: u16 = 3;

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

    // Grid coordinates are home-centered. Find the extent so we can center
    // the widget inside `area`.
    let (min_x, max_x, min_y, max_y) = grid_extent(grid);
    let width_units = max_x - min_x + 1.0;
    let height_units = max_y - min_y + 1.0;
    let widget_w = (width_units * CELL_W as f32).ceil() as u16;
    let widget_h = (height_units as u16) * CELL_H;

    let origin_x = area.x + area.width.saturating_sub(widget_w) / 2;
    let origin_y = area.y + area.height.saturating_sub(widget_h) / 2;

    let highlight_lower = highlight.map(|c| c.to_ascii_lowercase());

    for btn in &grid.buttons {
        let col = ((btn.x - min_x) * CELL_W as f32).round() as u16;
        let row = ((btn.y - min_y) as u16) * CELL_H;
        let x = origin_x + col;
        let y = origin_y + row;

        let is_highlighted = btn
            .mapping
            .as_ref()
            .and_then(|m| highlight_lower.map(|h| m.lower == h))
            .unwrap_or(false);

        render_button(f, x, y, area, btn, is_highlighted, heat);
    }
}

fn grid_extent(grid: &Grid) -> (f32, f32, f32, f32) {
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for b in &grid.buttons {
        min_x = min_x.min(b.x);
        max_x = max_x.max(b.x);
        min_y = min_y.min(b.y);
        max_y = max_y.max(b.y);
    }
    (min_x, max_x, min_y, max_y)
}

fn render_button(
    f: &mut Frame,
    x: u16,
    y: u16,
    area: Rect,
    btn: &GridButton,
    is_highlighted: bool,
    heat: Option<&Stats>,
) {
    if x + CELL_W > area.x + area.width || y + CELL_H > area.y + area.height {
        return;
    }
    let cell = Rect::new(x, y, CELL_W, CELL_H);

    let Some(mapping) = &btn.mapping else {
        // Unmapped key: dim outline, no character.
        let style = Style::default().fg(Color::DarkGray);
        let lines = vec![
            Line::from(Span::styled("┌───┐", style)),
            Line::from(Span::styled("│   │", style)),
            Line::from(Span::styled("└───┘", style)),
        ];
        f.render_widget(Paragraph::new(lines), cell);
        return;
    };

    // Heatmap mode: every key shows its heat color (gray when cold). This
    // keeps the F2 toggle visually meaningful even with no accumulated
    // data — pressing F2 should always *change* what you see.
    let color = match heat {
        Some(stats) => heatmap::heat_color(stats, mapping.lower).unwrap_or(Color::DarkGray),
        None => btn.finger.color(),
    };

    let lines = if is_highlighted {
        let border = Style::default().fg(color).bold();
        let letter = Style::default().fg(Color::White).bold();
        vec![
            Line::from(Span::styled("╔═══╗", border)),
            Line::from(vec![
                Span::styled("║ ", border),
                Span::styled(format!("{}", mapping.lower), letter),
                Span::styled(" ║", border),
            ]),
            Line::from(Span::styled("╚═══╝", border)),
        ]
    } else {
        let style = Style::default().fg(color);
        vec![
            Line::from(Span::styled("┌───┐", style)),
            Line::from(Span::styled(format!("│ {} │", mapping.lower), style)),
            Line::from(Span::styled("└───┘", style)),
        ]
    };

    f.render_widget(Paragraph::new(lines), cell);
}
