//! Shared UI components and layout helpers.

pub mod grid;
pub mod heatmap;

use ratatui::layout::{Constraint, Layout, Rect};

/// Standard vertically-centered content layout used by all modes.
pub struct ContentAreas {
    pub header: Rect,
    pub body: Rect,
    pub keyboard: Rect,
    pub stats: Rect,
    /// Footer line: small, muted — used for the keyboard/layout indicator.
    pub footer: Rect,
}

/// Build a centered layout with configurable body and keyboard heights.
/// header(1) + gap(1) + body(body_h) + gap(1) + keyboard(keyboard_h) + gap(1) + stats(1) + gap(1) + footer(1)
///
/// Callers pass the active grid's natural height via [`grid::grid_height`]
/// so staggered boards with splay get the vertical space they need
/// instead of getting their bottom rows culled by a fixed 12-line slot.
pub fn centered_content_layout(area: Rect, body_h: u16, keyboard_h: u16) -> ContentAreas {
    // Footer is two lines: line 1 is the keyboard/layout indicator,
    // line 2 is reserved for a broken-selection error message. The
    // second line stays blank when nothing's broken, preserving a
    // consistent vertical layout whether or not there's an error.
    let content_h: u16 = 1 + 1 + body_h + 1 + keyboard_h + 1 + 1 + 1 + 2;
    let [_, center, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(content_h),
        Constraint::Fill(1),
    ])
    .areas(area);

    let [header, _, body, _, keyboard, _, stats, _, footer] = Layout::vertical([
        Constraint::Length(1),          // header
        Constraint::Length(1),          // gap
        Constraint::Length(body_h),     // body
        Constraint::Length(1),          // gap
        Constraint::Length(keyboard_h), // keyboard
        Constraint::Length(1),          // gap
        Constraint::Length(1),          // stats
        Constraint::Length(1),          // gap
        Constraint::Length(2),          // footer (indicator + error line)
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

/// Render the standard two-line footer:
/// - line 1: dim `keyboard — layout` indicator with cycling hints. A
///   broken selection shows the offending name in red here.
/// - line 2: when a selection is broken, the parser's reason; blank
///   otherwise. Area is always 2 lines tall so the vertical layout
///   doesn't shift when an error appears.
pub fn render_footer(f: &mut ratatui::Frame, area: Rect, ctx: &crate::app::AppContext) {
    use ratatui::layout::Alignment;
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    let dim = Style::default().fg(Color::DarkGray);
    let red = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
    let red_dim = Style::default().fg(Color::Red);

    let g = ctx.grid();
    let mgr = ctx.grid_manager();

    let keyboard_span = match mgr.broken_keyboard() {
        Some(b) => Span::styled(b.name.clone(), red),
        None => Span::styled(g.keyboard_short.clone(), dim),
    };
    let layout_span = match mgr.broken_layout() {
        Some(b) => Span::styled(b.name.clone(), red),
        None => Span::styled(g.layout_short.clone(), dim),
    };

    let indicator = Line::from(vec![
        Span::styled("Ctrl+↑↓  ", dim),
        keyboard_span,
        Span::styled("  —  ", dim),
        layout_span,
        Span::styled("  Ctrl+←→", dim),
    ]);

    let error_line = match mgr
        .broken_keyboard()
        .map(|b| &b.reason)
        .or_else(|| mgr.broken_layout().map(|b| &b.reason))
    {
        Some(reason) => Line::from(Span::styled(truncate_reason(reason, 120), red_dim)),
        None => Line::from(""),
    };

    f.render_widget(
        Paragraph::new(vec![indicator, error_line]).alignment(Alignment::Center),
        area,
    );
}

/// Trim a reason string to at most `max` characters, adding `…` if cut.
/// Strips the `parsing <path>:` prefix that `configreader` adds so the
/// meaningful part (the field/line error) fits on a single line.
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
