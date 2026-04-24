//! P3 — Layout × You. "Does the layout work for me?"
//!
//! Two panels in v1:
//!
//! 1. **Finger load + hand balance** — per-finger count %, per-
//!    finger miss rate, L/R balance summary. Answers "is this
//!    layout balanced for *my* hands."
//! 2. **Usage over time** — mini keyboard rendered with the
//!    rank-based *usage* gradient for the current time bucket.
//!    Alt+←/→ walks buckets, so the user watches the shape-of-
//!    use drift across sessions, weeks, months.
//!
//! The page originally painted error heat on the keyboard, but
//! error heat goes dim once fluency kicks in — exactly when the
//! "does this layout work for me" question becomes worth asking.
//! Error heat still lives on the live typing view (F2 overlay
//! cycle); this page answers a longer-horizon question and usage
//! stays informative regardless of skill. Per-finger miss rate
//! in the panel above carries the error signal here.
//!
//! v2 (later, not here): drift cross-reference + roll analysis.

use std::collections::HashMap;

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use keywiz_stats::views::usage;

use crate::engine::state::FingerStats;
use crate::engine::Engine;
use crate::keyboard::common::Finger;
use crate::renderer::overlay::{UsageOverlay, UsageStyle};
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

    // ---- Usage keyboard ----
    f.render_widget(panel_header("Keyboard usage"), rects.get("heat_h"));
    draw_usage_keyboard(f, rects.get("keyboard"), engine);
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

/// Render a mini keyboard colored by the rank-based usage map for
/// the current filter scope.
///
/// The keyboard painted is the stats *combo's* keyboard+layout,
/// loaded from the catalog by name — not the live-typing pair.
/// So if you're typing on gallium-v2 right now but the stats combo
/// is drifter/halcyon_elora_v2, P3 shows drifter's letters on the
/// halcyon_elora_v2 geometry with drifter's usage data. When no
/// combo is set (the "all combos" scope), falls back to the live
/// pair.
fn draw_usage_keyboard(f: &mut Frame, area: Rect, engine: &Engine) {
    let Some(store) = engine.events_store() else {
        return;
    };
    let Some(filter) = engine.resolve_event_filter() else {
        return;
    };
    let map = usage::usage_map(store, &filter).unwrap_or_default();
    let overlay = UsageOverlay::new(map, UsageStyle::default());

    let placements = engine
        .stats_filter()
        .combo
        .as_ref()
        .and_then(|c| engine.placements_for_combo(&c.keyboard, &c.layout))
        .unwrap_or_else(|| engine.placements_for_terminal());
    // A minimal DisplayState lets render_keyboard run. It doesn't
    // read anything beyond `highlight_char`, which is unset here —
    // no "type this next" indicator belongs on a stats surface.
    let display = crate::engine::placement::DisplayState::default();
    // Stats surfaces never show a flash — flash is a live-typing
    // feedback mechanism, not a historical one.
    render_keyboard(f, area, &placements, &display, &overlay, None);
}
