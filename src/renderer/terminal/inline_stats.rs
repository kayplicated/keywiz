//! Inline stats view — the F3 alternative to the keyboard picture.
//!
//! Built for pro typers who've memorized the layout and want real-
//! time *diagnostics*, not a repeat of the footer's WPM/APM/acc
//! numbers. Every panel answers a question the footer can't:
//!
//! - **Consistency** — are my keystrokes evenly paced, or stuttery?
//! - **Burst** — am I still in my top gear?
//! - **Streak** — am I flowing, or breaking every few chars?
//! - **Hand balance** — is one hand carrying the session?
//! - **Recent trend** — is my accuracy climbing or drifting?
//! - **Weak now** — which bigram is actively dragging the session?
//! - **APM sparkline** — the shape of my last ~60s.
//!
//! All data reads from the live session via the engine's views. The
//! slot gets the keyboard's row budget (~12 rows for row-stag, ~10
//! for ortho); the layout below fits comfortably into that.

use std::collections::HashMap;

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, RenderDirection, Sparkline};
use ratatui::Frame;

use keywiz_stats::views::{bigram, rhythm};

use crate::engine::placement::DisplayState;
use crate::engine::Engine;
use crate::keyboard::common::Finger;
use crate::renderer::terminal::view::{Col, Row, View};

/// How many seconds of typing history the APM sparkline shows.
const SPARK_WINDOW_SECS: i64 = 60;
/// How many buckets the sparkline splits that window into. Chosen
/// so the rendered strip fits the centered sparkline area below.
const SPARK_BUCKETS: usize = 40;
/// Fraction of the page width the sparkline occupies, anchored
/// dead-center. `(0.25..0.75)` puts it between the middle of the
/// left column and the middle of the right column — roughly the
/// width of the center panel — so it reads as part of the grid.
const SPARK_L_PCT: u16 = 25;
const SPARK_R_PCT: u16 = 75;
/// How many keystrokes of "recent" history count toward the trend
/// arrow. ~30 keystrokes ≈ 6 seconds at 60 WPM — long enough to be
/// meaningful, short enough to respond to the user's current state.
const TREND_WINDOW: usize = 30;

pub fn draw(f: &mut Frame, area: Rect, display: &DisplayState, engine: &Engine) {
    let rects = View::page("inline_stats")
        // Row 1 — three panels: consistency, burst, streak.
        .add_row(
            Row::new("r1_h", 1).cols(vec![
                Col::fill("r1_h_a", 1),
                Col::fill("r1_h_b", 1),
                Col::fill("r1_h_c", 1),
            ]),
        )
        .add_row(
            Row::new("r1", 2).pad_bottom(1).cols(vec![
                Col::fill("r1_a", 1),
                Col::fill("r1_b", 1),
                Col::fill("r1_c", 1),
            ]),
        )
        // Row 2 — three panels: hand balance, trend, weak-now.
        .add_row(
            Row::new("r2_h", 1).cols(vec![
                Col::fill("r2_h_a", 1),
                Col::fill("r2_h_b", 1),
                Col::fill("r2_h_c", 1),
            ]),
        )
        .add_row(
            Row::new("r2", 2).pad_bottom(1).cols(vec![
                Col::fill("r2_a", 1),
                Col::fill("r2_b", 1),
                Col::fill("r2_c", 1),
            ]),
        )
        // Row 3 — APM sparkline, centered horizontally to roughly
        // the width of the middle panel column. Left/right padding
        // columns make the sparkline read as "belonging to the
        // grid" instead of drifting free.
        .add_row(
            Row::new("spark_h_row", 1).cols(vec![
                Col::percentage("spark_h_pad_l", SPARK_L_PCT),
                Col::percentage("spark_h", SPARK_R_PCT - SPARK_L_PCT),
                Col::percentage("spark_h_pad_r", 100 - SPARK_R_PCT),
            ]),
        )
        .add_row(
            Row::new("spark_row", 2).cols(vec![
                Col::percentage("spark_pad_l", SPARK_L_PCT),
                Col::percentage("spark", SPARK_R_PCT - SPARK_L_PCT),
                Col::percentage("spark_pad_r", 100 - SPARK_R_PCT),
            ]),
        )
        .resolve(area);

    // Pull event slice once; every panel reads it.
    let filter = engine.resolve_event_filter().unwrap_or_default();
    let events = engine
        .events_store()
        .and_then(|s| rhythm::collect_events(s, &filter).ok())
        .unwrap_or_default();

    // ---- Row 1: consistency / burst / streak ----
    f.render_widget(header("Consistency"), rects.get("r1_h_a"));
    f.render_widget(header("Burst"), rects.get("r1_h_b"));
    f.render_widget(header("Streak"), rects.get("r1_h_c"));

    f.render_widget(consistency_panel(&events), rects.get("r1_a"));
    f.render_widget(burst_panel(&events), rects.get("r1_b"));
    f.render_widget(streak_panel(&events), rects.get("r1_c"));

    // ---- Row 2: hand balance / trend / weak-now ----
    f.render_widget(header("Hands"), rects.get("r2_h_a"));
    f.render_widget(header("Recent"), rects.get("r2_h_b"));
    f.render_widget(header("Weak now"), rects.get("r2_h_c"));

    f.render_widget(hands_panel(engine), rects.get("r2_a"));
    f.render_widget(trend_panel(&events, display), rects.get("r2_b"));
    f.render_widget(weak_now_panel(engine), rects.get("r2_c"));

    // ---- Row 3: APM sparkline, centered ----
    f.render_widget(
        header(&format!("APM · last {SPARK_WINDOW_SECS}s")),
        rects.get("spark_h"),
    );
    let spark_rect = rects.get("spark");
    let spark_data = rolling_apm(&events);
    if spark_data.iter().sum::<u64>() == 0 {
        f.render_widget(
            muted("— keep typing —"),
            spark_rect,
        );
    } else {
        // `RenderDirection::RightToLeft` anchors the newest bucket
        // at the right edge and grows leftward, so the "present"
        // is always at the right and past recedes to the left.
        // When the data is shorter than the rect width, the
        // leftmost cells stay empty — the right read for "no
        // history there yet."
        let spark = Sparkline::default()
            .bar_set(symbols::bar::NINE_LEVELS)
            .direction(RenderDirection::RightToLeft)
            .data(&spark_data)
            .style(Style::default().fg(Color::Cyan));
        f.render_widget(spark, spark_rect);
    }
}

/// Small dim header line over a panel.
fn header(text: &str) -> Paragraph<'static> {
    Paragraph::new(Line::from(Span::styled(
        text.to_string(),
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center)
}

fn muted(text: &str) -> Paragraph<'static> {
    Paragraph::new(Line::from(Span::styled(
        text.to_string(),
        Style::default().fg(Color::DarkGray),
    )))
    .alignment(Alignment::Center)
}

/// Coefficient-of-variation % + a hint line. Lower = smoother.
fn consistency_panel(events: &[keywiz_stats::Event]) -> Paragraph<'static> {
    let cv = rhythm::consistency_pct(events);
    let (value_line, style) = match cv {
        Some(pct) => (
            format!("{:.0}%", pct),
            Style::default().fg(cv_color(pct)).add_modifier(Modifier::BOLD),
        ),
        None => ("—".to_string(), Style::default().fg(Color::DarkGray)),
    };
    Paragraph::new(vec![
        Line::from(Span::styled(value_line, style)),
        Line::from(Span::styled(
            "lower = smoother",
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .alignment(Alignment::Center)
}

fn cv_color(pct: f64) -> Color {
    if pct < 30.0 {
        Color::Green
    } else if pct < 55.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Burst WPM = peak rolling 5s. Subtitle: "peak 5s".
fn burst_panel(events: &[keywiz_stats::Event]) -> Paragraph<'static> {
    let burst = rhythm::burst_wpm(events, 5_000);
    let (value, style) = if burst > 0.0 {
        (
            format!("{:.0}", burst),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )
    } else {
        (
            "—".to_string(),
            Style::default().fg(Color::DarkGray),
        )
    };
    Paragraph::new(vec![
        Line::from(Span::styled(value, style)),
        Line::from(Span::styled(
            "peak 5s",
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .alignment(Alignment::Center)
}

/// Current correct-in-a-row + session's best-ever streak.
fn streak_panel(events: &[keywiz_stats::Event]) -> Paragraph<'static> {
    let (current, best) = streak_numbers(events);
    let value_style = if current > 0 {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Paragraph::new(vec![
        Line::from(Span::styled(format!("{current}"), value_style)),
        Line::from(Span::styled(
            format!("best {best}"),
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .alignment(Alignment::Center)
}

/// Walk the session forward; return (current run at the end, best
/// run seen). Ignores session boundaries — this panel is about the
/// live typing run, not the entire event stream.
fn streak_numbers(events: &[keywiz_stats::Event]) -> (u32, u32) {
    let mut current = 0u32;
    let mut best = 0u32;
    for ev in events {
        if ev.correct {
            current += 1;
            if current > best {
                best = current;
            }
        } else {
            current = 0;
        }
    }
    (current, best)
}

/// Two-row bar: `L 48% ▓▓▓▓░` / `R 52% ▓▓▓▓▓`. Asymmetry > 10%
/// colors the dominant side yellow — a hint that the session is
/// leaning on one hand.
fn hands_panel(engine: &Engine) -> Paragraph<'static> {
    let load = engine.finger_load();
    let (l, r) = split_hands(&load);
    let total = l + r;
    if total == 0 {
        return muted("—");
    }
    let l_pct = (l as f64 / total as f64) * 100.0;
    let r_pct = (r as f64 / total as f64) * 100.0;
    let l_style = hand_color(l_pct, r_pct);
    let r_style = hand_color(r_pct, l_pct);

    Paragraph::new(vec![
        hand_line("L", l_pct, l_style),
        hand_line("R", r_pct, r_style),
    ])
    .alignment(Alignment::Center)
}

fn split_hands(load: &HashMap<Finger, crate::engine::state::FingerStats>) -> (u64, u64) {
    let mut l = 0u64;
    let mut r = 0u64;
    for (f, s) in load {
        match f {
            Finger::LPinky | Finger::LRing | Finger::LMiddle | Finger::LIndex | Finger::LThumb => {
                l += s.count;
            }
            Finger::RPinky | Finger::RRing | Finger::RMiddle | Finger::RIndex | Finger::RThumb => {
                r += s.count;
            }
        }
    }
    (l, r)
}

fn hand_color(own: f64, other: f64) -> Style {
    if (own - other).abs() > 10.0 && own > other {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    }
}

/// `L 48% ▓▓▓▓▓░░░░░` — percentage plus a 10-cell bar. Bar uses
/// block chars because they render at any font size, and the
/// length is the same on every row so both hands align vertically.
fn hand_line(label: &str, pct: f64, pct_style: Style) -> Line<'static> {
    let filled = ((pct / 10.0).round() as usize).min(10);
    let bar: String = std::iter::repeat_n('▓', filled)
        .chain(std::iter::repeat_n('░', 10 - filled))
        .collect();
    Line::from(vec![
        Span::styled(
            format!("{label} "),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(format!("{:>3}%", pct.round() as i64), pct_style),
        Span::raw(" "),
        Span::styled(bar, Style::default().fg(Color::Cyan)),
    ])
}

/// Recent accuracy vs session accuracy. Arrow + delta pp.
fn trend_panel(
    events: &[keywiz_stats::Event],
    display: &DisplayState,
) -> Paragraph<'static> {
    if events.len() < TREND_WINDOW {
        return Paragraph::new(vec![
            Line::from(Span::styled(
                "→",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "warming up",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .alignment(Alignment::Center);
    }
    let recent = &events[events.len() - TREND_WINDOW..];
    let recent_correct = recent.iter().filter(|e| e.correct).count();
    let recent_pct = (recent_correct as f64 / TREND_WINDOW as f64) * 100.0;
    let session_pct = display.session_accuracy;
    let delta = recent_pct - session_pct;

    // Using ±0.5pp as the flat-arrow threshold: small enough that a
    // single hit/miss in the 30-keystroke window (≈3.3pp swing)
    // still moves the arrow, not so small that rounding flutter
    // kicks it around.
    let (arrow, color) = if delta > 0.5 {
        ("▲", Color::Green)
    } else if delta < -0.5 {
        ("▼", Color::Red)
    } else {
        ("→", Color::DarkGray)
    };

    let delta_label = if delta.abs() < 0.05 {
        "stable".to_string()
    } else {
        format!("{:+.0}pp", delta)
    };

    Paragraph::new(vec![
        Line::from(Span::styled(
            arrow,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            delta_label,
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .alignment(Alignment::Center)
}

/// Worst bigram in the current scope, live.
fn weak_now_panel(engine: &Engine) -> Paragraph<'static> {
    let Some(store) = engine.events_store() else {
        return muted("—");
    };
    let Some(filter) = engine.resolve_event_filter() else {
        return muted("—");
    };
    let pairs = bigram::worst_bigrams(store, &filter, 3).unwrap_or_default();
    let Some(((a, b), stats)) = pairs.into_iter().next() else {
        return Paragraph::new(vec![
            Line::from(Span::styled(
                "·",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(Span::styled(
                "clean so far",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .alignment(Alignment::Center);
    };
    let pct = (stats.miss_rate() * 100.0).round() as i64;
    Paragraph::new(vec![
        Line::from(Span::styled(
            format!("{a}{b}"),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("{pct}% miss"),
            Style::default().fg(miss_color(stats.miss_rate())),
        )),
    ])
    .alignment(Alignment::Center)
}

fn miss_color(rate: f64) -> Color {
    if rate < 0.08 {
        Color::Green
    } else if rate < 0.20 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Bucket the last `SPARK_WINDOW_SECS` of events into a vec where
/// **index 0 = most recent, index N-1 = oldest**. That ordering
/// matches `RenderDirection::RightToLeft` on ratatui's sparkline:
/// the renderer draws index 0 at the right edge and walks
/// leftward, so the present always sits on the right and the past
/// trails off to the left.
///
/// The window ends at "now" as inferred from the newest event's
/// timestamp. When typing history is shorter than the window, the
/// far-left (oldest) buckets stay zero — the eye reads empty
/// cells there as "nothing happened yet," which is correct.
fn rolling_apm(events: &[keywiz_stats::Event]) -> Vec<u64> {
    let mut buckets = vec![0u64; SPARK_BUCKETS];
    if events.is_empty() {
        return buckets;
    }
    let last_ts = events.iter().map(|e| e.ts_ms).max().unwrap_or(0);
    let window_ms = SPARK_WINDOW_SECS * 1000;
    let from_ts = last_ts - window_ms;
    let bucket_span = (window_ms as f64 / SPARK_BUCKETS as f64).max(1.0);
    for ev in events {
        if ev.ts_ms < from_ts {
            continue;
        }
        // Invert the offset so the newest events land at index 0.
        let reverse_offset = (last_ts - ev.ts_ms) as f64;
        let idx = (reverse_offset / bucket_span).floor() as usize;
        let idx = idx.min(SPARK_BUCKETS - 1);
        buckets[idx] += 1;
    }
    buckets
}
