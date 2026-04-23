//! F5 layout-iterations modal — "how is the layout performing."
//!
//! F4 answers "how am I typing" (performance across time, per-page
//! views into the event stream). F5 answers a different question:
//! "how has this layout's *content* evolved, and did each iteration
//! actually help?"
//!
//! A layout's identity is its content hash. Every time the user
//! swaps keys, the hash changes. Sessions carry both the hash
//! (authoritative identity) and the display name. This modal
//! groups all sessions under the current combo's layout name by
//! hash, oldest → newest, and shows headline numbers per
//! iteration.
//!
//! Requires a specific combo to be selected in the F4 stats
//! filter (iterations only make sense within one layout name +
//! one keyboard). When none is selected, or when only a single
//! iteration exists, a muted empty state explains what to do.

use chrono::{DateTime, Local};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::engine::state::IterationStats;
use crate::engine::Engine;
use crate::renderer::terminal::view::{Row, View};

pub fn draw(f: &mut Frame, area: Rect, engine: &Engine) {
    let rects = View::page("layout_page")
        .add_row(Row::new("header", 1).pad_bottom(1))
        .add_row(Row::new("subheader", 1).pad_bottom(1))
        .add_row(Row::new("body", 14).pad_bottom(1))
        .add_row(Row::new("footer", 2))
        .resolve(area);

    let filter = engine.stats_filter();
    let combo_label = match &filter.combo {
        Some(c) => format!("{} / {}", pretty(&c.layout), pretty(&c.keyboard)),
        None => "—".to_string(),
    };

    // Header — page title.
    let header = Line::from(Span::styled(
        "Layout iterations",
        Style::default().fg(Color::Cyan).bold(),
    ));
    f.render_widget(
        Paragraph::new(header).alignment(Alignment::Center),
        rects.get("header"),
    );
    // Subheader — which combo we're looking at.
    let subheader = Line::from(Span::styled(
        combo_label,
        Style::default().fg(Color::Gray),
    ));
    f.render_widget(
        Paragraph::new(subheader).alignment(Alignment::Center),
        rects.get("subheader"),
    );

    // Body — dispatch on what the engine can tell us.
    match engine.iteration_stats() {
        None => draw_empty(
            f,
            rects.get("body"),
            "Pick a combo first. Open F4, cycle Ctrl+←/→ until the footer shows \
             a specific (layout / keyboard) pair, then come back here.",
        ),
        Some(iters) if iters.is_empty() => draw_empty(
            f,
            rects.get("body"),
            "No sessions recorded for this combo yet.",
        ),
        Some(iters) if iters.len() == 1 => {
            // Single-iteration case — show the one row but also a
            // note explaining how iterations come to be.
            draw_iterations(f, rects.get("body"), &iters, true);
        }
        Some(iters) => draw_iterations(f, rects.get("body"), &iters, false),
    }

    // Footer — exit hint, matches the app grammar.
    let footer = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "F5 or ESC to close",
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .alignment(Alignment::Center);
    f.render_widget(footer, rects.get("footer"));
}

fn draw_empty(f: &mut Frame, area: Rect, message: &str) {
    let p = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            message.to_string(),
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .alignment(Alignment::Center);
    f.render_widget(p, area);
}

fn draw_iterations(
    f: &mut Frame,
    area: Rect,
    iters: &[IterationStats],
    single_only: bool,
) {
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(iters.len() + 4);

    lines.push(table_header());
    lines.push(Line::from(""));

    // Most recent at the top — matches the user's mental model
    // ("current iteration first, older ones below").
    for (idx, it) in iters.iter().enumerate().rev() {
        // Ordinal is 1-based in natural order (oldest = #1).
        let ordinal = idx + 1;
        lines.push(iteration_line(ordinal, it));
    }

    if single_only {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Only one iteration on record. Swap keys in the layout \
             JSON to create a new content hash — future sessions \
             land under a new row here.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    f.render_widget(
        Paragraph::new(lines).alignment(Alignment::Left),
        area,
    );
}

fn table_header() -> Line<'static> {
    let dim = Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD);
    let col = |label: &str, width: usize| Span::styled(format!("{label:<width$}"), dim);
    Line::from(vec![
        col("#", 4),
        col("hash", 11),
        col("dates", 22),
        col("sessions", 10),
        col("keys", 10),
        col("WPM", 6),
        col("Acc", 6),
    ])
}

fn iteration_line(ordinal: usize, it: &IterationStats) -> Line<'static> {
    let gray = Style::default().fg(Color::Gray);
    let cyan = Style::default().fg(Color::Cyan);
    let dim = Style::default().fg(Color::DarkGray);

    let short_hash: String = it.hash.0.chars().take(8).collect();
    let dates = format_date_range(it.first_seen_ms, it.last_seen_ms);
    let wpm = format!("{:.0}", it.net_wpm());
    let acc_pct = it.accuracy_pct();
    let acc_style = Style::default().fg(acc_color(acc_pct));

    Line::from(vec![
        Span::styled(format!("#{:<3}", ordinal), gray),
        Span::styled(format!("{:<11}", short_hash), dim),
        Span::styled(format!("{:<22}", dates), gray),
        Span::styled(format!("{:<10}", it.session_count), gray),
        Span::styled(format!("{:<10}", it.total_events), gray),
        Span::styled(format!("{wpm:<6}"), cyan),
        Span::styled(format!("{:.0}%", acc_pct).to_string(), acc_style),
    ])
}

fn format_date_range(first_ms: i64, last_ms: i64) -> String {
    let first = DateTime::from_timestamp_millis(first_ms)
        .map(|dt| dt.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "—".to_string());
    let last = DateTime::from_timestamp_millis(last_ms)
        .map(|dt| dt.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "—".to_string());
    if first == last {
        first
    } else {
        format!("{first} → {last}")
    }
}

fn acc_color(pct: f64) -> Color {
    if pct >= 95.0 {
        Color::Green
    } else if pct >= 85.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Match the stats footer's name-prettifier so the combo label
/// reads the same across modals.
fn pretty(raw: &str) -> String {
    raw.replace('_', " ")
}
