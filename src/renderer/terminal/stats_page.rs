//! F4 full-screen stats modal.
//!
//! Replaces every other surface. Typing is paused upstream. Uses
//! the [`View`](crate::renderer::terminal::view::View) builder so
//! the page shares centering, column-width, and padding grammar
//! with the typing view — changing those properties in one place
//! updates both surfaces at once.
//!
//! Three pages: Overview (P1), Progression (P2), Layout × You
//! (P3). The dispatcher here routes on `display.stats_view`
//! (stringly-typed so adding a page is a new file + one match arm).
//! Pages read from `&Engine` directly so they can run ad-hoc view
//! queries over arbitrary time slices / layouts — the live typing
//! surfaces stay on the pre-composed-data contract.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::engine::placement::DisplayState;
use crate::engine::stats_filter::Granularity;
use crate::engine::Engine;
use crate::renderer::terminal::view::{Row, View};

pub mod layout_view;
pub mod overview;
pub mod progression;

/// How many rows the body gets. Sized to fit the overview page:
/// 3 (numbers) + 1+4 (rhythm/peak) + 1+5 (worst tables) + 1+2
/// (sparkline) + inter-block padding. Bump when a page needs more;
/// the View builder clamps overflow into the *next* row (the
/// footer), so undersizing this shows up as visual bleed rather
/// than truncation.
const BODY_ROWS: u16 = 20;

pub fn draw(f: &mut Frame, area: Rect, display: &DisplayState, engine: &Engine) {
    let rects = View::page("stats")
        .add_row(Row::new("header", 1).pad_bottom(1))
        .add_row(Row::new("body", BODY_ROWS).pad_bottom(1))
        .add_row(Row::new("footer", 3))
        .resolve(area);

    // Header — page title.
    let view_title = match display.stats_view {
        "progression" => "Progression",
        "layout_view" => "Layout × You",
        _ => "Overview",
    };
    let header = Line::from(Span::styled(
        format!("Stats — {view_title}"),
        Style::default().fg(Color::Cyan).bold(),
    ));
    f.render_widget(
        Paragraph::new(header).alignment(Alignment::Center),
        rects.get("header"),
    );

    // Body — dispatch to the active page.
    match display.stats_view {
        "progression" => progression::draw(f, rects.get("body"), engine),
        "layout_view" => layout_view::draw(f, rects.get("body"), engine),
        _ => overview::draw(f, rects.get("body"), display, engine),
    }

    // Footer — key hints + live scope values, mirroring the typing
    // view's `Ctrl+↑↓ · Halcyon Elora v2` grammar. Second line is
    // the exit hint.
    render_footer(f, rects.get("footer"), display, engine);
}

/// Two-line footer matching the typing view's `binding · value`
/// shape. Line 1: the four cycle bindings each paired with the
/// live scope value. Line 2: exit hint.
fn render_footer(f: &mut Frame, area: Rect, display: &DisplayState, engine: &Engine) {
    let dim = Style::default().fg(Color::DarkGray);
    let name = Style::default().fg(Color::Gray);

    let sep = Span::styled("   ", dim);
    let dot = Span::styled(" · ", dim);

    let page_label = match display.stats_view {
        "progression" => "Progression",
        "layout_view" => "Layout × You",
        _ => "Overview",
    };

    let filter = engine.stats_filter();
    let combo_label = match &filter.combo {
        Some(c) => format!("{} / {}", pretty_name(&c.layout), pretty_name(&c.keyboard)),
        None => "all combos".to_string(),
    };
    let granularity_label = granularity_short(filter.granularity);
    let offset_label = if display.stats_view == "progression" {
        progression_range_label(filter.granularity, filter.range)
    } else {
        offset_short(filter.granularity, filter.offset)
    };

    // Line 1: combo + page. Line 2: granularity + offset. Line 3:
    // exit hint. Splitting across lines means the 90-column view
    // doesn't truncate the offset label when a combo name is long.
    let row_nav = Line::from(vec![
        Span::styled("Ctrl+←→", dim),
        dot.clone(),
        Span::styled(combo_label, name),
        sep.clone(),
        Span::styled("Ctrl+↑↓", dim),
        dot.clone(),
        Span::styled(page_label.to_string(), name),
    ]);
    let row_time = Line::from(vec![
        Span::styled("Alt+↑↓", dim),
        dot.clone(),
        Span::styled(granularity_label.to_string(), name),
        sep,
        Span::styled("Alt+←→", dim),
        dot,
        Span::styled(offset_label, name),
    ]);
    let exit = Line::from(Span::styled("F4 or ESC to close", dim));

    f.render_widget(
        Paragraph::new(vec![row_nav, row_time, exit]).alignment(Alignment::Center),
        area,
    );
}

/// Cheap display-prettifier: swap underscores for spaces. Source
/// names (`halcyon_elora_v2`, `gallium-v2`) are code-friendly;
/// the footer reads better with human spacing.
fn pretty_name(raw: &str) -> String {
    raw.replace('_', " ")
}

fn granularity_short(g: Granularity) -> &'static str {
    match g {
        Granularity::CurrentSession => "session",
        Granularity::Day => "day",
        Granularity::Week => "week",
        Granularity::Month => "month",
        Granularity::Year => "year",
        Granularity::All => "all time",
    }
}

/// Compact offset label for the footer. Reads `now` for offset 0,
/// `-N` for earlier buckets; hidden (`—`) when the granularity
/// doesn't use offsets (CurrentSession / All).
fn offset_short(g: Granularity, offset: i64) -> String {
    match g {
        Granularity::CurrentSession | Granularity::All => "—".to_string(),
        _ if offset == 0 => "now".to_string(),
        _ => format!("{offset}"),
    }
}

/// On P2 the Alt+←/→ axis controls range-width, not offset. Show
/// `last 7 days` / `last 14 weeks` etc. so the footer tells the
/// user what the axis *does* right now. `—` when the current
/// granularity has no multi-bucket view.
fn progression_range_label(g: Granularity, range: usize) -> String {
    let unit = match g {
        Granularity::Day => "days",
        Granularity::Week => "weeks",
        Granularity::Month => "months",
        Granularity::Year => "years",
        Granularity::CurrentSession | Granularity::All => return "—".to_string(),
    };
    format!("last {range} {unit}")
}
