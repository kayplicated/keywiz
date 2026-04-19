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

/// Build a centered layout with configurable body height.
/// header(1) + gap(1) + body(body_h) + gap(1) + keyboard(12) + gap(1) + stats(1) + gap(1) + footer(1)
pub fn centered_content_layout(area: Rect, body_h: u16) -> ContentAreas {
    let content_h: u16 = 1 + 1 + body_h + 1 + 12 + 1 + 1 + 1 + 1;
    let [_, center, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(content_h),
        Constraint::Fill(1),
    ])
    .areas(area);

    let [header, _, body, _, keyboard, _, stats, _, footer] = Layout::vertical([
        Constraint::Length(1),      // header
        Constraint::Length(1),      // gap
        Constraint::Length(body_h), // body
        Constraint::Length(1),      // gap
        Constraint::Length(12),     // keyboard
        Constraint::Length(1),      // gap
        Constraint::Length(1),      // stats
        Constraint::Length(1),      // gap
        Constraint::Length(1),      // footer
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

/// Render the standard footer: dim `keyboard — layout` indicator, centered.
/// Safe to call from every mode; falls back to the legacy layout name when
/// no grid manager is present.
pub fn render_footer(f: &mut ratatui::Frame, area: Rect, ctx: &crate::app::AppContext) {
    use ratatui::layout::Alignment;
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    let g = ctx.grid();
    let text = format!("{} — {}", g.keyboard_short, g.layout_short);
    let para = Paragraph::new(Line::from(Span::styled(
        text,
        Style::default().fg(Color::DarkGray),
    )))
    .alignment(Alignment::Center);
    f.render_widget(para, area);
}
