//! P3 — Layout × You. "Does the layout work for me?"
//!
//! Two panels in v1:
//!
//! 1. **Finger load + hand balance** — per-finger count %, per-
//!    finger miss rate, L/R balance summary. Answers "is this
//!    layout balanced for *my* hands."
//! 2. **Heat over time** — mini keyboard rendered with heat colors
//!    for the current time bucket. Alt+←/→ walks buckets, so the
//!    user watches hot keys cool off as they practice. Progress
//!    made visible.
//!
//! v2 (later, not here): drift cross-reference + roll analysis.

use std::collections::HashMap;

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use keywiz_stats::views::heat;

use crate::engine::state::FingerStats;
use crate::engine::Engine;
use crate::keyboard::common::Finger;
use crate::renderer::overlay::{HeatOverlay, HeatStyle};
use crate::renderer::terminal::render_keyboard;
use crate::renderer::terminal::view::{Col, Row, View};

pub fn draw(f: &mut Frame, area: Rect, engine: &Engine) {
    let rects = View::page("layout_view")
        .add_row(
            Row::new("hands_h", 1).cols(vec![
                Col::fill("hands_lh", 1),
                Col::fill("hands_rh", 1),
            ]),
        )
        .add_row(
            Row::new("hands", 5).pad_bottom(1).cols(vec![
                Col::fill("hands_l", 1),
                Col::fill("hands_r", 1),
            ]),
        )
        .add_row(Row::new("heat_h", 1))
        .add_row(Row::new("keyboard", 12))
        .resolve(area);

    // ---- Finger load panels ----
    let load = engine.finger_load();
    f.render_widget(
        panel_header("Left hand"),
        rects.get("hands_lh"),
    );
    f.render_widget(
        panel_header("Right hand"),
        rects.get("hands_rh"),
    );
    let total: u64 = load.values().map(|s| s.count).sum();
    f.render_widget(
        hand_panel(&load, total, true),
        rects.get("hands_l"),
    );
    f.render_widget(
        hand_panel(&load, total, false),
        rects.get("hands_r"),
    );

    // ---- Heat keyboard ----
    f.render_widget(panel_header("Keyboard heat"), rects.get("heat_h"));
    draw_heat_keyboard(f, rects.get("keyboard"), engine);
}

/// Paragraph header helper — darkgray bold title.
fn panel_header(text: &str) -> Paragraph<'static> {
    Paragraph::new(Line::from(Span::styled(
        text.to_string(),
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
}

/// One hand's stats — four finger rows + a summary line with the
/// hand's total share.
fn hand_panel(
    load: &HashMap<Finger, FingerStats>,
    total: u64,
    left: bool,
) -> Paragraph<'static> {
    let fingers: &[(Finger, &str)] = if left {
        &[
            (Finger::LPinky, "pinky "),
            (Finger::LRing, "ring  "),
            (Finger::LMiddle, "middle"),
            (Finger::LIndex, "index "),
        ]
    } else {
        &[
            (Finger::RIndex, "index "),
            (Finger::RMiddle, "middle"),
            (Finger::RRing, "ring  "),
            (Finger::RPinky, "pinky "),
        ]
    };

    let hand_total: u64 = fingers
        .iter()
        .map(|(f, _)| load.get(f).map(|s| s.count).unwrap_or(0))
        .sum();

    let mut lines: Vec<Line<'static>> = fingers
        .iter()
        .map(|(f, label)| finger_line(load, *f, label, total))
        .collect();

    let hand_pct = if total == 0 {
        0.0
    } else {
        (hand_total as f64 / total as f64) * 100.0
    };
    lines.push(Line::from(vec![
        Span::styled(
            if left { "L " } else { "R " },
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("{:>5}%", format!("{:.0}", hand_pct)),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("   ({hand_total} keys)"),
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    Paragraph::new(lines).alignment(Alignment::Center)
}

/// Single finger row — `pinky   12% · 8% miss · 248 keys`.
fn finger_line(
    load: &HashMap<Finger, FingerStats>,
    finger: Finger,
    label: &str,
    total: u64,
) -> Line<'static> {
    let stats = load.get(&finger).copied().unwrap_or_default();
    let load_pct = if total == 0 {
        0.0
    } else {
        (stats.count as f64 / total as f64) * 100.0
    };
    let miss_pct = stats.miss_rate() * 100.0;

    let dim = Style::default().fg(Color::DarkGray);
    let miss_style = Style::default().fg(miss_color(stats.miss_rate()));
    Line::from(vec![
        Span::styled(format!("{label} "), dim),
        Span::styled(
            format!("{:>3}%", load_pct.round() as i64),
            Style::default().fg(Color::Gray),
        ),
        Span::styled("   ", dim),
        Span::styled(
            format!("{:>3}% miss", miss_pct.round() as i64),
            miss_style,
        ),
    ])
}

fn miss_color(rate: f64) -> Color {
    if rate < 0.05 {
        Color::Green
    } else if rate < 0.15 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Render a mini keyboard colored by heat for the current filter
/// scope. Reuses the main-view keyboard renderer with a temporary
/// `HeatOverlay` built from the scoped event slice.
fn draw_heat_keyboard(f: &mut Frame, area: Rect, engine: &Engine) {
    let Some(store) = engine.events_store() else {
        return;
    };
    let Some(filter) = engine.resolve_event_filter() else {
        return;
    };
    let map = heat::heat_map(store, &filter).unwrap_or_default();
    let overlay = HeatOverlay::new(map, HeatStyle::default());

    // Build placements off the live keyboard + layout. We'd prefer
    // the filtered combo's own snapshot here (so P3 reflects the
    // layout you're analyzing even when you've since switched
    // away), but turning a canonical-JSON snapshot back into a
    // `dyn Keyboard` needs a loader that doesn't exist yet. P3 v1
    // uses the live keyboard; cross-layout heat-over-time waits
    // for snapshot-based placement rendering.
    let placements = engine.placements_for_terminal();
    // A minimal DisplayState lets render_keyboard run. It doesn't
    // read anything beyond `highlight_char`, which is unset here —
    // no "type this next" indicator belongs on a stats surface.
    let display = crate::engine::placement::DisplayState::default();
    // Stats surfaces never show a flash — flash is a live-typing
    // feedback mechanism, not a historical one.
    render_keyboard(f, area, &placements, &display, &overlay, None);
}
