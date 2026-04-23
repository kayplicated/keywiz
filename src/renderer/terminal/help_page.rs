//! F1 help page — static keybind reference.
//!
//! Same modal pattern as [`stats_page`](super::stats_page): typing
//! pauses upstream, this just paints a grouped keybind table.
//! Groups are Pages, Display, Cycling, Control — mirroring how
//! the footer + main loop conceptually partition input.
//!
//! Stateless. `display` isn't consulted (the reference doesn't
//! change across app state) but is accepted for signature
//! consistency with the other page renderers.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::engine::placement::DisplayState;
use crate::renderer::terminal::view::{Row, View};

/// How many rows the keybind table needs. Tune when rows are
/// added or removed.
const BODY_ROWS: u16 = 20;

pub fn draw(f: &mut Frame, area: Rect, _display: &DisplayState) {
    let rects = View::page("help")
        .add_row(Row::new("header", 1).pad_bottom(1))
        .add_row(Row::new("body", BODY_ROWS).pad_bottom(1))
        .add_row(Row::new("footer", 1))
        .resolve(area);

    let header = Line::from(Span::styled(
        "Keybinds",
        Style::default().fg(Color::Cyan).bold(),
    ));
    f.render_widget(
        Paragraph::new(header).alignment(Alignment::Center),
        rects.get("header"),
    );

    let body = Paragraph::new(keybind_lines()).alignment(Alignment::Left);
    f.render_widget(body, rects.get("body"));

    let footer = Line::from(Span::styled(
        "F1 or ESC to close",
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(
        Paragraph::new(footer).alignment(Alignment::Center),
        rects.get("footer"),
    );
}

fn keybind_lines() -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    push_group(&mut lines, "Pages");
    push_row(&mut lines, "F1", "Toggle this help page");
    push_row(&mut lines, "F4", "Toggle stats page (how am I typing)");
    push_row(&mut lines, "F5", "Toggle layout iterations (how is the layout)");
    push_blank(&mut lines);

    push_group(&mut lines, "Display");
    push_row(&mut lines, "Tab", "Hide / show the keyboard slot");
    push_row(&mut lines, "Shift+Tab", "Toggle flash on keypress");
    push_row(&mut lines, "F2", "Cycle overlay (none / finger / heat)");
    push_row(&mut lines, "F3", "Cycle slot content (keyboard / inline stats)");
    push_blank(&mut lines);

    push_group(&mut lines, "Cycling");
    push_row(&mut lines, "Ctrl+↑ / ↓", "Previous / next keyboard");
    push_row(&mut lines, "Ctrl+← / →", "Previous / next layout");
    push_row(&mut lines, "Alt+↑ / ↓", "Previous / next exercise category");
    push_row(&mut lines, "Alt+← / →", "Previous / next exercise instance");
    push_blank(&mut lines);

    push_group(&mut lines, "Control");
    push_row(&mut lines, "Esc", "Quit (or close an open modal)");

    lines
}

fn push_group(lines: &mut Vec<Line<'static>>, label: &str) {
    lines.push(Line::from(Span::styled(
        label.to_string(),
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    )));
}

fn push_row(lines: &mut Vec<Line<'static>>, key: &str, description: &str) {
    let key_span = Span::styled(
        format!("  {key:<18}"),
        Style::default().fg(Color::Cyan),
    );
    let desc_span = Span::styled(
        description.to_string(),
        Style::default().fg(Color::Gray),
    );
    lines.push(Line::from(vec![key_span, desc_span]));
}

fn push_blank(lines: &mut Vec<Line<'static>>) {
    lines.push(Line::from(""));
}
