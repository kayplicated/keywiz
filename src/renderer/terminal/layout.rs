//! Page-level layout helpers for terminal modes.
//!
//! `centered_content_layout` carves a mode's screen into header /
//! body / keyboard / stats / footer bands. `render_footer` paints
//! the standard two-line footer from a `DisplayState`.

use ratatui::layout::{Constraint, Layout, Rect};

use crate::engine::placement::DisplayState;

pub struct ContentAreas {
    pub header: Rect,
    pub body: Rect,
    pub keyboard: Rect,
    pub stats: Rect,
    pub footer: Rect,
}

pub fn centered_content_layout(area: Rect, body_h: u16, keyboard_h: u16) -> ContentAreas {
    // Footer is two lines: indicator + optional error message.
    let content_h: u16 = 1 + 1 + body_h + 1 + keyboard_h + 1 + 1 + 1 + 2;
    let [_, center, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(content_h),
        Constraint::Fill(1),
    ])
    .areas(area);

    let [header, _, body, _, keyboard, _, stats, _, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(body_h),
        Constraint::Length(1),
        Constraint::Length(keyboard_h),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(2),
    ])
    .areas(center);

    ContentAreas {
        header,
        body,
        keyboard,
        stats,
        footer,
    }
}

/// Two-line footer. Line 1: keyboard — layout — exercise indicator
/// with cycling hints (broken names show in red). Line 2: parse
/// error reason when something's broken, blank otherwise.
pub fn render_footer(f: &mut ratatui::Frame, area: Rect, display: &DisplayState) {
    use ratatui::layout::Alignment;
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    let dim = Style::default().fg(Color::DarkGray);
    let red = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
    let red_dim = Style::default().fg(Color::Red);

    let name = Style::default().fg(Color::Gray);

    let keyboard_name = match &display.broken_keyboard {
        Some(b) => Span::styled(b.name.clone(), red),
        None => Span::styled(display.keyboard_short.clone(), name),
    };
    let layout_name = match &display.broken_layout {
        Some(b) => Span::styled(b.name.clone(), red),
        None => Span::styled(display.layout_short.clone(), name),
    };
    let exercise_name = Span::styled(display.exercise_short.clone(), name);

    // Each group: "Ctrl+↑↓ · Keyboard"; groups separated by "   —   "
    // so the (binding, name) pairs read as discrete units.
    let sep = Span::styled("     ", dim);
    let dot = Span::styled(" · ", dim);

    let indicator = Line::from(vec![
        Span::styled("Ctrl+↑↓", dim),
        dot.clone(),
        keyboard_name,
        sep.clone(),
        Span::styled("Ctrl+←→", dim),
        dot.clone(),
        layout_name,
        sep,
        Span::styled("Alt+←→", dim),
        dot,
        exercise_name,
    ]);

    let reason = display
        .broken_keyboard
        .as_ref()
        .map(|b| &b.reason)
        .or_else(|| display.broken_layout.as_ref().map(|b| &b.reason));

    let error_line = match reason {
        Some(reason) => Line::from(Span::styled(truncate_reason(reason, 120), red_dim)),
        None => Line::from(""),
    };

    f.render_widget(
        Paragraph::new(vec![indicator, error_line]).alignment(Alignment::Center),
        area,
    );
}

fn truncate_reason(reason: &str, max: usize) -> String {
    let trimmed = reason.find(':').map_or(reason, |i| &reason[i + 1..]).trim();
    if trimmed.chars().count() <= max {
        trimmed.to_string()
    } else {
        let mut s: String = trimmed.chars().take(max - 1).collect();
        s.push('…');
        s
    }
}
