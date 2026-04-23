//! P1 — Overview. Dense single-page snapshot of "what's going on
//! right now" within the active stats filter scope.
//!
//! Every row has to tell the user something the live footer doesn't
//! already show. The footer carries WPM / APM / correct / wrong /
//! accuracy; this page goes deeper into rhythm, peak, per-key and
//! per-bigram diagnostics, and paints an APM sparkline across the
//! time window.
//!
//! Reads events directly from `engine.events_store()` using the
//! resolved [`EventFilter`](keywiz_stats::EventFilter) so changing
//! the filter re-scopes every panel.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Sparkline};
use ratatui::Frame;

use keywiz_stats::views::{bigram, keys, rhythm, wpm};

use crate::engine::placement::DisplayState;
use crate::engine::Engine;
use crate::renderer::terminal::view::{Col, Row, View};

/// Minimum occurrences before a bigram is eligible for the
/// worst-bigrams table — single-occurrence flukes don't belong
/// at the top of a diagnostic.
const BIGRAM_MIN_COUNT: u64 = 3;
const KEY_MIN_COUNT: u64 = 3;
const WORST_LIMIT: usize = 5;
/// Warmup / end segment size for the session arc column. A session
/// needs at least 3 × this many events for the split to produce
/// non-overlapping segments.
const ARC_WINDOW: usize = 30;

pub fn draw(f: &mut Frame, area: Rect, display: &DisplayState, engine: &Engine) {
    let rects = View::page("overview")
        .add_row(
            Row::new("numbers", 3).pad_bottom(1).cols(vec![
                Col::fill("num_a", 1),
                Col::fill("num_b", 1),
                Col::fill("num_c", 1),
            ]),
        )
        .add_row(
            Row::new("detail_h", 1).cols(vec![
                Col::fill("rhythm_h", 1),
                Col::fill("peak_h", 1),
                Col::fill("arc_h", 1),
            ]),
        )
        .add_row(
            Row::new("detail", 4).pad_bottom(1).cols(vec![
                Col::fill("rhythm", 1),
                Col::fill("peak", 1),
                Col::fill("arc", 1),
            ]),
        )
        .add_row(
            Row::new("worst_h", 1).cols(vec![
                Col::fill("worst_bigrams_h", 1),
                Col::fill("worst_keys_h", 1),
            ]),
        )
        .add_row(
            Row::new("worst", 5).pad_bottom(1).cols(vec![
                Col::fill("worst_bigrams", 1),
                Col::fill("worst_keys", 1),
            ]),
        )
        .add_row(Row::new("spark_h", 1))
        .add_row(Row::new("sparkline", 2))
        .resolve(area);

    // Pull the event slice once and share it across every panel.
    // All four headline numbers, rhythm, peak, and the sparkline
    // read from this slice so everything agrees on the active
    // filter scope. Bigram + keys views run their own store
    // queries since they need session-grouped aggregation.
    let events = fetch_events(engine).unwrap_or_default();
    let speed = speed_from_events(&events);

    // ---- Row 1: headline numbers (WPM, APM, Accuracy + counts) ----
    //
    // Yes, these overlap the footer. They're the anchoring context
    // for everything below — every other block on this page is read
    // against "you're typing at X WPM, Y% accuracy."
    f.render_widget(
        big_number("WPM", speed.net_wpm().round() as i64, Color::Cyan),
        rects.get("num_a"),
    );
    f.render_widget(
        big_number("APM", speed.apm().round() as i64, Color::Cyan),
        rects.get("num_b"),
    );
    f.render_widget(
        accuracy_block(&speed),
        rects.get("num_c"),
    );
    // `display` informs nothing on this page — future per-exercise
    // breakdowns might want it, but the scoped view supersedes the
    // live-session data for every current panel.
    let _ = display;

    // ---- Rows 2–3: rhythm + peak + session-arc details ----
    f.render_widget(panel_header("Rhythm"), rects.get("rhythm_h"));
    f.render_widget(panel_header("Peak"), rects.get("peak_h"));
    f.render_widget(panel_header("Session arc"), rects.get("arc_h"));

    let (rhythm_block, peak_block, arc_block, sparkline_data) = if events.is_empty() {
        (
            muted_paragraph(vec!["median   —", "p95      —", "consist. —"]),
            muted_paragraph(vec!["burst WPM —", "streak    —", "fastest   —"]),
            muted_paragraph(vec!["warmup —", "steady —", "end    —"]),
            Vec::new(),
        )
    } else {
        (
            rhythm_paragraph(&events),
            peak_paragraph(&events),
            arc_paragraph(&events),
            rhythm::apm_buckets(&events, 40),
        )
    };
    f.render_widget(rhythm_block, rects.get("rhythm"));
    f.render_widget(peak_block, rects.get("peak"));
    f.render_widget(arc_block, rects.get("arc"));

    // ---- Rows 4–5: worst bigrams + worst keys ----
    f.render_widget(panel_header("Worst bigrams"), rects.get("worst_bigrams_h"));
    f.render_widget(panel_header("Worst keys"), rects.get("worst_keys_h"));
    f.render_widget(
        worst_bigrams_paragraph(engine),
        rects.get("worst_bigrams"),
    );
    f.render_widget(worst_keys_paragraph(engine), rects.get("worst_keys"));

    // ---- Row 6: APM sparkline ----
    f.render_widget(panel_header("APM over time"), rects.get("spark_h"));
    if sparkline_data.is_empty() {
        f.render_widget(
            muted_paragraph(vec!["— no timing data yet —"]),
            rects.get("sparkline"),
        );
    } else {
        let data: Vec<u64> = sparkline_data.iter().map(|v| v.round() as u64).collect();
        let spark = Sparkline::default()
            .bar_set(symbols::bar::NINE_LEVELS)
            .data(&data)
            .style(Style::default().fg(Color::Cyan));
        f.render_widget(spark, rects.get("sparkline"));
    }
}

/// Big centered "LABEL / value" block.
fn big_number(label: &str, value: i64, color: Color) -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(Span::styled(
            label.to_string(),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("{value}"),
            Style::default().fg(color).bold(),
        )),
    ])
    .alignment(Alignment::Center)
}

/// Accuracy cell — accuracy % on one line, correct/wrong counts on
/// the next. Tighter than a pure big-number since the counts
/// matter *with* the accuracy (95% of 40 keystrokes is a different
/// signal from 95% of 4000).
fn accuracy_block(speed: &wpm::SessionWpm) -> Paragraph<'static> {
    let wrong = speed.total_keystrokes.saturating_sub(speed.correct_keystrokes);
    let acc_pct = if speed.total_keystrokes == 0 {
        100.0
    } else {
        (speed.correct_keystrokes as f64 / speed.total_keystrokes as f64) * 100.0
    };
    Paragraph::new(vec![
        Line::from(Span::styled(
            "Accuracy",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("{:.0}%", acc_pct),
            Style::default().fg(Color::Yellow).bold(),
        )),
        Line::from(vec![
            Span::styled(
                format!("{}", speed.correct_keystrokes),
                Style::default().fg(Color::Green),
            ),
            Span::styled(" / ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{wrong}"),
                Style::default().fg(Color::Red),
            ),
        ]),
    ])
    .alignment(Alignment::Center)
}

/// Tally WPM / APM / accuracy numbers from the scoped event slice.
/// Mirrors `views::wpm::live_for` but for an arbitrary filter — the
/// page uses this so headline numbers respect the active filter
/// (not just the live session).
fn speed_from_events(events: &[keywiz_stats::Event]) -> wpm::SessionWpm {
    let mut speed = wpm::SessionWpm::default();
    for ev in events {
        speed.total_keystrokes += 1;
        if ev.correct {
            speed.correct_keystrokes += 1;
        }
        if let Some(ms) = ev.delta_ms {
            speed.active_ms += ms as u64;
        }
    }
    speed
}

/// Small panel-header line — a block title in darkgray.
fn panel_header(text: &str) -> Paragraph<'static> {
    Paragraph::new(Line::from(Span::styled(
        text.to_string(),
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
}

fn rhythm_paragraph(events: &[keywiz_stats::Event]) -> Paragraph<'static> {
    let median = rhythm::median_delta_ms(events);
    let p95 = rhythm::p95_delta_ms(events);
    let consistency = rhythm::consistency_pct(events);
    let lines = vec![
        kv_line("median", median.map(|v| format!("{:.0} ms", v))),
        kv_line("p95", p95.map(|v| format!("{:.0} ms", v))),
        kv_line(
            "consist.",
            consistency.map(|v| format!("{:.0}%", v)),
        ),
    ];
    Paragraph::new(lines).alignment(Alignment::Center)
}

fn peak_paragraph(events: &[keywiz_stats::Event]) -> Paragraph<'static> {
    let burst = rhythm::burst_wpm(events, 5_000);
    let streak = rhythm::longest_correct_streak(events);
    let fastest = rhythm::fastest_delta_ms(events);
    let lines = vec![
        kv_line(
            "burst WPM",
            if burst > 0.0 {
                Some(format!("{:.0}", burst))
            } else {
                None
            },
        ),
        kv_line(
            "streak",
            if streak > 0 {
                Some(format!("{streak}"))
            } else {
                None
            },
        ),
        kv_line("fastest", fastest.map(|ms| format!("{ms} ms"))),
    ];
    Paragraph::new(lines).alignment(Alignment::Center)
}

/// Warmup / steady / end WPM across the active session. Empty
/// until the session has at least 3 * `ARC_WINDOW` keystrokes —
/// before then the three segments would overlap and the split
/// carries no signal.
///
/// The end-value color signals fatigue at a glance:
/// - green when end ≥ steady (still going strong)
/// - yellow when end < steady * 0.9 (noticeably slower)
/// - gray (neutral) otherwise
fn arc_paragraph(events: &[keywiz_stats::Event]) -> Paragraph<'static> {
    let warmup = rhythm::warmup_wpm(events, ARC_WINDOW);
    let steady = rhythm::steady_wpm(events, ARC_WINDOW);
    let end = rhythm::end_wpm(events, ARC_WINDOW);

    let end_color = match (steady, end) {
        (Some(s), Some(e)) if e >= s => Color::Green,
        (Some(s), Some(e)) if e < s * 0.9 => Color::Yellow,
        _ => Color::Gray,
    };

    let lines = vec![
        kv_line("warmup", warmup.map(|v| format!("{:.0}", v))),
        kv_line("steady", steady.map(|v| format!("{:.0}", v))),
        end_line("end", end, end_color),
    ];
    Paragraph::new(lines).alignment(Alignment::Center)
}

/// Variant of `kv_line` that accepts a value-color, used for the
/// session arc's "end" row so fatigue reads at a glance.
fn end_line(label: &str, value: Option<f64>, color: Color) -> Line<'static> {
    let value_text = value
        .map(|v| format!("{:.0}", v))
        .unwrap_or_else(|| "—".to_string());
    Line::from(vec![
        Span::styled(
            format!("{label:<9} "),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(value_text, Style::default().fg(color)),
    ])
}

/// Worst-bigrams table: `th   22%   (11/50)`.
fn worst_bigrams_paragraph(engine: &Engine) -> Paragraph<'static> {
    let Some(store) = engine.events_store() else {
        return muted_paragraph(vec!["— no events store —"]);
    };
    let Some(filter) = engine.resolve_event_filter() else {
        return muted_paragraph(vec!["— no active session —"]);
    };
    let pairs = match bigram::worst_bigrams(store, &filter, BIGRAM_MIN_COUNT) {
        Ok(p) => p,
        Err(_) => return muted_paragraph(vec!["— query failed —"]),
    };
    if pairs.is_empty() {
        return muted_paragraph(vec!["— keep typing —"]);
    }
    let mut lines = Vec::with_capacity(WORST_LIMIT);
    for ((a, b), stats) in pairs.into_iter().take(WORST_LIMIT) {
        let pct = (stats.miss_rate() * 100.0).round() as i64;
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}{}", a, b),
                Style::default().fg(Color::White).bold(),
            ),
            Span::raw("   "),
            Span::styled(
                format!("{pct}%"),
                Style::default().fg(miss_color(stats.miss_rate())),
            ),
            Span::styled(
                format!("   ({}/{})", stats.miss_count, stats.count),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }
    Paragraph::new(lines).alignment(Alignment::Center)
}

/// Worst-keys table.
fn worst_keys_paragraph(engine: &Engine) -> Paragraph<'static> {
    let Some(store) = engine.events_store() else {
        return muted_paragraph(vec!["— no events store —"]);
    };
    let Some(filter) = engine.resolve_event_filter() else {
        return muted_paragraph(vec!["— no active session —"]);
    };
    let sorted = match keys::worst_keys(store, &filter, KEY_MIN_COUNT) {
        Ok(s) => s,
        Err(_) => return muted_paragraph(vec!["— query failed —"]),
    };
    if sorted.is_empty() {
        return muted_paragraph(vec!["— keep typing —"]);
    }
    let mut lines = Vec::with_capacity(WORST_LIMIT);
    for (ch, stats) in sorted.into_iter().take(WORST_LIMIT) {
        let pct = (stats.miss_rate() * 100.0).round() as i64;
        let display_char = if ch == ' ' { '␣' } else { ch };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}", display_char),
                Style::default().fg(Color::White).bold(),
            ),
            Span::raw("    "),
            Span::styled(
                format!("{pct}%"),
                Style::default().fg(miss_color(stats.miss_rate())),
            ),
            Span::styled(
                format!("   ({}/{})", stats.miss_count, stats.count),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    }
    Paragraph::new(lines).alignment(Alignment::Center)
}

/// `label   value` line; value `—` when `None`.
fn kv_line(label: &str, value: Option<String>) -> Line<'static> {
    let value_text = value.unwrap_or_else(|| "—".to_string());
    Line::from(vec![
        Span::styled(
            format!("{label:<9} "),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(value_text, Style::default().fg(Color::Gray)),
    ])
}

fn muted_paragraph(lines: Vec<&str>) -> Paragraph<'static> {
    let lines: Vec<Line<'static>> = lines
        .into_iter()
        .map(|s| {
            Line::from(Span::styled(
                s.to_string(),
                Style::default().fg(Color::DarkGray),
            ))
        })
        .collect();
    Paragraph::new(lines).alignment(Alignment::Center)
}

/// Color ramp for miss rate: green → yellow → red. Mirrors the
/// live stats line's choice of green for good, red for bad so the
/// two surfaces agree.
fn miss_color(rate: f64) -> Color {
    if rate < 0.08 {
        Color::Green
    } else if rate < 0.20 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Pull the event vector once for the active filter. Returns an
/// empty vec when no events store is open or the filter resolves
/// to nothing. Callers guard against empty upstream.
fn fetch_events(engine: &Engine) -> Option<Vec<keywiz_stats::Event>> {
    let store = engine.events_store()?;
    let filter = engine.resolve_event_filter()?;
    let mut events = Vec::new();
    for ev in store.events(&filter).ok()?.flatten() {
        events.push(ev);
    }
    Some(events)
}
