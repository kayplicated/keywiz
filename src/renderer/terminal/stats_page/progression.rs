//! P2 — Progression. "Am I getting better?"
//!
//! Renders N consecutive buckets at the chosen granularity as a
//! compact table (bucket label · WPM · APM · accuracy · keystrokes)
//! plus a WPM sparkline at the top so the trend reads at a glance.
//!
//! Alt+←/→ widen/narrow the plotted range on this page
//! (context-appropriate reuse of the P1/P3 offset keys).
//! Granularity = CurrentSession or All produce empty output —
//! progression is meaningless without a bucket axis.

use chrono::{DateTime, Datelike, Local};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Sparkline};
use ratatui::Frame;

use keywiz_stats::views::progression::BucketStats;

use crate::engine::stats_filter::Granularity;
use crate::engine::Engine;
use crate::renderer::terminal::view::{Row, View};

pub fn draw(f: &mut Frame, area: Rect, engine: &Engine) {
    let granularity = engine.stats_filter().granularity;

    // Single-bucket granularities don't have a progression.
    if matches!(granularity, Granularity::CurrentSession | Granularity::All) {
        draw_empty(
            f,
            area,
            "Switch granularity with Alt+↑↓ (day / week / month / year) to see progression.",
        );
        return;
    }

    let buckets = engine.progression_buckets();
    if buckets.is_empty() {
        draw_empty(f, area, "No data in the active scope yet. Keep typing.");
        return;
    }

    let rects = View::page("progression")
        .add_row(Row::new("spark_h", 1))
        .add_row(Row::new("sparkline", 3).pad_bottom(1))
        .add_row(Row::new("table_h", 1))
        .add_row(Row::new("table", 12))
        .resolve(area);

    // ---- WPM sparkline ----
    let spark_data: Vec<u64> =
        buckets.iter().map(|b| b.net_wpm().round() as u64).collect();
    let peak = spark_data.iter().max().copied().unwrap_or(0).max(1);
    let spark_header = Paragraph::new(Line::from(vec![
        Span::styled(
            "WPM trend",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(
            format!("peak {peak}"),
            Style::default().fg(Color::DarkGray),
        ),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(spark_header, rects.get("spark_h"));

    let spark = Sparkline::default()
        .bar_set(symbols::bar::NINE_LEVELS)
        .data(&spark_data)
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(spark, rects.get("sparkline"));

    // ---- Table header + rows ----
    let table_header = Paragraph::new(table_header_line(granularity))
        .alignment(Alignment::Left);
    f.render_widget(table_header, rects.get("table_h"));

    let max_rows = rects.get("table").height as usize;
    // Show the most recent buckets first (user's mental model is
    // "today is at the top"), but the series we got is oldest-first.
    // Reverse-iter and take as many as fit.
    let lines: Vec<Line<'static>> = buckets
        .iter()
        .rev()
        .take(max_rows)
        .map(|b| bucket_row(granularity, b))
        .collect();
    f.render_widget(
        Paragraph::new(lines).alignment(Alignment::Left),
        rects.get("table"),
    );
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

fn table_header_line(g: Granularity) -> Line<'static> {
    let dim = Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD);
    let col = |label: &str, width: usize| Span::styled(format!("{label:<width$}"), dim);
    Line::from(vec![
        col(bucket_col_name(g), 14),
        col("WPM", 7),
        col("APM", 7),
        col("Acc", 7),
        col("Keys", 8),
        col("Active", 8),
    ])
}

fn bucket_col_name(g: Granularity) -> &'static str {
    match g {
        Granularity::Day => "Day",
        Granularity::Week => "Week of",
        Granularity::Month => "Month",
        Granularity::Year => "Year",
        _ => "Bucket",
    }
}

fn bucket_row(g: Granularity, b: &BucketStats) -> Line<'static> {
    let label_style = Style::default().fg(Color::Gray);
    let num_style = Style::default().fg(Color::Cyan);
    let dim = Style::default().fg(Color::DarkGray);

    let label = format_bucket_label(g, b.from_ms);

    if b.is_empty() {
        return Line::from(vec![
            Span::styled(format!("{label:<14}"), label_style),
            Span::styled(format!("{:<7}", "—"), dim),
            Span::styled(format!("{:<7}", "—"), dim),
            Span::styled(format!("{:<7}", "—"), dim),
            Span::styled(format!("{:<8}", "—"), dim),
            Span::styled(format!("{:<8}", "—"), dim),
        ]);
    }

    let acc = b.accuracy_pct();
    let acc_style = Style::default().fg(acc_color(acc));
    let active = format_duration(b.active_ms);

    Line::from(vec![
        Span::styled(format!("{label:<14}"), label_style),
        Span::styled(format!("{:<7}", format!("{:.0}", b.net_wpm())), num_style),
        Span::styled(format!("{:<7}", format!("{:.0}", b.apm())), num_style),
        Span::styled(format!("{:<7}", format!("{:.0}%", acc)), acc_style),
        Span::styled(format!("{:<8}", b.total_events), label_style),
        Span::styled(format!("{:<8}", active), dim),
    ])
}

/// Human label for a bucket's start timestamp at the given
/// granularity. Day → `Mon 04-22`, Week → `04-15`, Month →
/// `Apr 2026`, Year → `2026`.
fn format_bucket_label(g: Granularity, from_ms: i64) -> String {
    let Some(utc) = DateTime::from_timestamp_millis(from_ms) else {
        return "—".to_string();
    };
    let local: DateTime<Local> = utc.with_timezone(&Local);
    match g {
        Granularity::Day => local.format("%a %m-%d").to_string(),
        Granularity::Week => local.format("%m-%d").to_string(),
        Granularity::Month => local.format("%b %Y").to_string(),
        Granularity::Year => local.year().to_string(),
        _ => "—".to_string(),
    }
}

/// `mm:ss` duration — fits better in a narrow column than decimal
/// minutes. Rounds down at the second boundary; we're not doing
/// stopwatch precision.
fn format_duration(active_ms: u64) -> String {
    let total_s = active_ms / 1000;
    let mins = total_s / 60;
    let secs = total_s % 60;
    if mins >= 60 {
        let hours = mins / 60;
        format!("{hours}h{:02}m", mins % 60)
    } else {
        format!("{mins}m{secs:02}s")
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
