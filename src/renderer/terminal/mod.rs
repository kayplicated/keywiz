//! Terminal renderer.
//!
//! Draws placements + display state. Does not read the keyboard,
//! layout, stats, or exercise — the engine hands it pre-composed
//! data. The renderer's job is to turn `Placement` and
//! `DisplayState` into ratatui widgets.
//!
//! Terminal interprets placements as:
//! - `pos_a` = column (key-grid units, integer-valued f32)
//! - `pos_b` = row   (key-grid units, integer-valued f32)
//! - `pos_r` = ignored (gui honors rotation)
//!
//! Multiplied by [`CELL_W`] / [`CELL_H`] to reach terminal cells.

pub mod body;
pub mod draw;
pub mod heatmap;
pub mod layout;
pub mod naming;

pub use layout::{centered_content_layout, render_footer, ContentAreas};

use ratatui::layout::Rect;
use ratatui::Frame;

use crate::engine::placement::{DisplayState, Placement};

/// Terminal cells per one unit of `pos_a` (one key-width).
pub const CELL_W: u16 = 5;
/// Terminal cells per one unit of `pos_b` (one key-height).
pub const CELL_H: u16 = 3;

/// Draw a complete frame from engine-provided data. This is the
/// single entry point main.rs calls.
pub fn draw_frame(f: &mut ratatui::Frame, placements: &[Placement], display: &DisplayState) {
    use ratatui::layout::Alignment;
    use ratatui::style::{Color, Style, Stylize};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    let body_h: u16 = if display.text.is_some() { 10 } else { 3 };
    let kb_h = if display.keyboard_visible {
        placements_height(placements)
    } else {
        0
    };
    let areas = centered_content_layout(f.area(), body_h, kb_h);

    // ---- header ----
    let header_text = if let Some(level) = &display.drill_level {
        Line::from(vec![
            Span::styled("Drill", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" — "),
            Span::styled(level.clone(), Style::default().fg(Color::Yellow)),
        ])
    } else if let Some(words) = &display.words {
        let suffix = match words.target_count {
            Some(n) => format!(" — {}/{}", words.word_index, n),
            None => format!(" — {} words", words.word_index),
        };
        Line::from(vec![
            Span::styled(
                "Typing Practice",
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::raw(suffix),
        ])
    } else if display.text.is_some() {
        Line::from(Span::styled(
            "Text Practice",
            Style::default().fg(Color::Cyan).bold(),
        ))
    } else {
        Line::from("")
    };
    f.render_widget(Paragraph::new(header_text).alignment(Alignment::Center), areas.header);

    // ---- body ----
    body::draw_body(f, areas.body, display);

    // ---- keyboard ----
    if display.keyboard_visible {
        render_keyboard(f, areas.keyboard, placements, display);
    }

    // ---- stats ----
    let stats_line = Line::from(vec![
        Span::styled(
            format!("Correct: {}", display.session_total_correct),
            Style::default().fg(Color::Green),
        ),
        Span::raw("  "),
        Span::styled(
            format!("Wrong: {}", display.session_total_wrong),
            Style::default().fg(Color::Red),
        ),
        Span::raw("  "),
        Span::styled(
            format!("Accuracy: {:.0}%", display.session_accuracy),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw("  "),
        Span::styled(
            if display.keyboard_visible {
                "TAB hide keyboard"
            } else {
                "TAB show keyboard"
            },
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw("  "),
        Span::styled("ESC to quit", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(stats_line).alignment(Alignment::Center), areas.stats);

    // ---- footer ----
    render_footer(f, areas.footer, display);
}

/// Terminal height in lines needed to render a set of placements.
/// Modes pass this into `centered_content_layout` so staggered and
/// thumb-clustered boards get enough vertical space.
pub fn placements_height(placements: &[Placement]) -> u16 {
    if placements.is_empty() {
        return 0;
    }
    let mut min_b = f32::INFINITY;
    let mut max_b = f32::NEG_INFINITY;
    for p in placements {
        min_b = min_b.min(p.pos_b);
        max_b = max_b.max(p.pos_b + p.height);
    }
    ((max_b - min_b) * CELL_H as f32).ceil() as u16
}

/// Render placements centered within `area`. `display` provides
/// the highlight target and heatmap toggle.
pub fn render_keyboard(
    f: &mut Frame,
    area: Rect,
    placements: &[Placement],
    display: &DisplayState,
) {
    if area.width < 3 || area.height < 3 || placements.is_empty() {
        return;
    }

    // Compute the widget's pixel bounds so we can center it.
    let mut min_col = i32::MAX;
    let mut max_col = i32::MIN;
    let mut min_row = i32::MAX;
    let mut max_row = i32::MIN;
    for p in placements {
        let col = (p.pos_a * CELL_W as f32).round() as i32;
        let row = (p.pos_b * CELL_H as f32).round() as i32;
        let w = (p.width * CELL_W as f32).round().max(3.0) as i32;
        let h = (p.height * CELL_H as f32).round().max(3.0) as i32;
        min_col = min_col.min(col);
        max_col = max_col.max(col + w);
        min_row = min_row.min(row);
        max_row = max_row.max(row + h);
    }
    let widget_w = (max_col - min_col).max(0) as u16;
    let widget_h = (max_row - min_row).max(0) as u16;

    let origin_x = area.x + area.width.saturating_sub(widget_w) / 2;
    let origin_y = area.y + area.height.saturating_sub(widget_h) / 2;

    let highlight_lower = display.highlight_char.map(|c| c.to_ascii_lowercase());

    for placement in placements {
        let col = (placement.pos_a * CELL_W as f32).round() as i32;
        let row = (placement.pos_b * CELL_H as f32).round() as i32;
        let x = origin_x as i32 + col - min_col;
        let y = origin_y as i32 + row - min_row;
        if x < 0 || y < 0 {
            continue;
        }
        let x = x as u16;
        let y = y as u16;
        let w = (placement.width * CELL_W as f32).round().max(3.0) as u16;
        let h = (placement.height * CELL_H as f32).round().max(3.0) as u16;
        if x + w > area.x + area.width || y + h > area.y + area.height {
            continue;
        }
        let rect = Rect::new(x, y, w, h);

        // Highlight matching: the engine hands us a char to
        // highlight, renderer compares to each placement's label
        // (for Char mappings, label is the lower char).
        let is_highlighted = match highlight_lower {
            Some(h) => placement.label.chars().next() == Some(h),
            None => false,
        };

        draw::draw_key(f, rect, placement, is_highlighted, display.heatmap_visible);
    }
}
