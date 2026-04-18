//! Shared UI components and layout helpers.

pub mod heatmap;
pub mod keyboard;

use ratatui::layout::{Constraint, Layout, Rect};

/// Standard vertically-centered content layout used by all modes.
pub struct ContentAreas {
    pub header: Rect,
    pub body: Rect,
    pub keyboard: Rect,
    pub stats: Rect,
}

/// Build a centered layout with configurable body height.
/// header(1) + gap(1) + body(body_h) + gap(1) + keyboard(12) + gap(1) + stats(1)
pub fn centered_content_layout(area: Rect, body_h: u16) -> ContentAreas {
    let content_h: u16 = 1 + 1 + body_h + 1 + 12 + 1 + 1;
    let [_, center, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(content_h),
        Constraint::Fill(1),
    ])
    .areas(area);

    let [header, _, body, _, keyboard, _, stats] = Layout::vertical([
        Constraint::Length(1),      // header
        Constraint::Length(1),      // gap
        Constraint::Length(body_h), // body
        Constraint::Length(1),      // gap
        Constraint::Length(12),     // keyboard
        Constraint::Length(1),      // gap
        Constraint::Length(1),      // stats
    ])
    .areas(center);

    ContentAreas {
        header,
        body,
        keyboard,
        stats,
    }
}
