//! Terminal renderer.
//!
//! Draws placements + display state. Does not read the keyboard,
//! layout, stats, or exercise — the engine hands it pre-composed
//! data. The renderer's job is to turn `Placement` and
//! `DisplayState` into ratatui widgets.
//!
//! Terminal interprets placements as:
//! - `pos_a` = column (key-grid units, integer-valued f32)
//! - `pos_b` = row   (key-grid units, integer-valued f32)
//! - `pos_r` = ignored (gui honors rotation)
//!
//! Multiplied by [`CELL_W`] / [`CELL_H`] to reach terminal cells.

pub mod body;
pub mod draw;
pub mod heatmap;
pub mod help_page;
pub mod inline_stats;
pub mod layout;
pub mod layout_page;
pub mod naming;
pub mod stats_page;
pub mod view;

pub use layout::render_footer;

use ratatui::layout::Rect;
use ratatui::Frame;

use ratatui::style::Color;

use crate::engine::placement::{DisplayState, Placement};
use crate::renderer::terminal::view::{Row, View};

/// Terminal cells per one unit of `pos_a` (one key-width).
pub const CELL_W: u16 = 5;
/// Terminal cells per one unit of `pos_b` (one key-height).
pub const CELL_H: u16 = 3;

/// Max age (ms) a flash keystroke is still visible. Past this the
/// flash layer reads the key as "no flash here."
const FLASH_FADE_MS: u64 = 250;

/// One frame's worth of flash state, resolved from
/// `Engine::last_flash()` + `Engine::flash_enabled()`. Rendered as
/// a bright border on the matching key, intensity stepping down
/// as the keystroke ages.
#[derive(Debug, Clone, Copy)]
pub struct FlashFrame {
    pub char: char,
    pub age_ms: u64,
}

/// Draw a complete frame from engine-provided data. This is the
/// single entry point main.rs calls.
///
/// The stats modal reaches into `&Engine` directly so its pages can
/// run ad-hoc view queries (over arbitrary time slices, arbitrary
/// layouts) that don't fit the fixed shape of `DisplayState`. The
/// live typing surfaces stay on the pre-composed-data contract.
pub fn draw_frame(
    f: &mut ratatui::Frame,
    placements: &[Placement],
    display: &DisplayState,
    overlay: &dyn crate::renderer::overlay::KeyOverlay,
    engine: &crate::engine::Engine,
) {
    use ratatui::layout::Alignment;
    use ratatui::style::{Color, Style, Stylize};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    // F1 help page: modal, replaces everything.
    if display.help_page_visible {
        help_page::draw(f, f.area(), display);
        return;
    }

    // F4 stats page: modal, replaces everything.
    if display.stats_page_visible {
        stats_page::draw(f, f.area(), display, engine);
        return;
    }

    // F5 layout-iterations page: modal, replaces everything.
    if display.layout_page_visible {
        layout_page::draw(f, f.area(), engine);
        return;
    }

    let body_h: u16 = if display.text.is_some() { 10 } else { 3 };
    let kb_h = if display.slot_visible {
        placements_height(placements)
    } else {
        0
    };

    let rects = View::page("typing")
        .add_row(Row::new("header", 1).pad_bottom(1))
        .add_row(Row::new("body", body_h).pad_bottom(1))
        .add_row(Row::new("slot", kb_h).pad_bottom(1))
        .add_row(Row::new("stats_line", 1).pad_bottom(1))
        .add_row(Row::new("footer", 2))
        .resolve(f.area());

    // ---- header ----
    let header_text = if let Some(level) = &display.drill_level {
        Line::from(vec![
            Span::styled("Drill", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" — "),
            Span::styled(level.clone(), Style::default().fg(Color::Yellow)),
        ])
    } else if let Some(words) = &display.words {
        Line::from(vec![
            Span::styled(
                "Typing Practice",
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::raw(format!(" — {} words", words.word_index)),
        ])
    } else if let Some(text) = &display.text {
        // Position (n/m) and the switch hint live in the footer;
        // header carries only the passage title so the eye has
        // one less thing to parse while reading.
        Line::from(vec![Span::styled(
            text.title.clone(),
            Style::default().fg(Color::Cyan).bold(),
        )])
    } else {
        Line::from("")
    };
    f.render_widget(
        Paragraph::new(header_text).alignment(Alignment::Center),
        rects.get("header"),
    );

    // ---- body ----
    body::draw_body(f, rects.get("body"), display);

    // ---- keyboard slot (keyboard or inline stats) ----
    if display.slot_visible {
        let slot_rect = rects.get("slot");
        match display.slot {
            "inline_stats" => inline_stats::draw(f, slot_rect, display, engine),
            _ => {
                let flash = current_flash(engine);
                render_keyboard(
                    f,
                    slot_rect,
                    placements,
                    display,
                    overlay,
                    flash.as_ref(),
                );
            }
        }
    }

    // ---- stats line ----
    //
    // Live performance numbers + layered state indicators. Overlay
    // (F2) and flash (Shift+Tab) state gets rendered here so the
    // user can see what's on without having to remember the
    // toggle's state.
    let dim = Style::default().fg(Color::DarkGray);
    let mut stats_spans = vec![
        Span::styled(
            format!("WPM: {:.0}", display.session_wpm),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("  "),
        Span::styled(
            format!("APM: {:.0}", display.session_apm),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw("  "),
        Span::styled(
            format!("Correct: {}", display.session_total_correct),
            Style::default().fg(Color::Green),
        ),
        Span::raw("  "),
        Span::styled(
            format!("Wrong: {}", display.session_total_wrong),
            Style::default().fg(Color::Red),
        ),
        Span::raw("  "),
        Span::styled(
            format!("Accuracy: {:.0}%", display.session_accuracy),
            Style::default().fg(Color::Yellow),
        ),
    ];
    // Overlay indicator — only surface non-default modes. "none"
    // is the default and would just clutter the strip.
    if display.overlay_name != "none" {
        stats_spans.push(Span::styled("    overlay: ", dim));
        stats_spans.push(Span::styled(
            display.overlay_name.to_string(),
            Style::default().fg(overlay_indicator_color(display.overlay_name)),
        ));
    }
    // Flash indicator — only when on. The F2-overlay pattern
    // applies: "off" is the default, signalling it would be
    // distracting noise.
    if engine.flash_enabled() {
        stats_spans.push(Span::styled("    flash: ", dim));
        stats_spans.push(Span::styled(
            "on",
            Style::default().fg(Color::Magenta),
        ));
    }
    stats_spans.push(Span::raw("  "));
    stats_spans.push(Span::styled("F1 help", dim));
    f.render_widget(
        Paragraph::new(Line::from(stats_spans)).alignment(Alignment::Center),
        rects.get("stats_line"),
    );

    // ---- footer ----
    render_footer(f, rects.get("footer"), display);
}

/// Terminal height in lines needed to render a set of placements.
pub fn placements_height(placements: &[Placement]) -> u16 {
    if placements.is_empty() {
        return 0;
    }
    let mut min_b = f32::INFINITY;
    let mut max_b = f32::NEG_INFINITY;
    for p in placements {
        min_b = min_b.min(p.pos_b);
        max_b = max_b.max(p.pos_b + p.height);
    }
    ((max_b - min_b) * CELL_H as f32).ceil() as u16
}

/// Pick a color for the overlay indicator in the stats line.
/// Matches the overlay's dominant visual so the footer reads in
/// the same palette as the keyboard it describes.
fn overlay_indicator_color(name: &str) -> Color {
    match name {
        "heat" => Color::Red,
        "finger" => Color::Cyan,
        _ => Color::Gray,
    }
}

/// Return a `FlashFrame` for the current engine state, or `None`
/// when flash is disabled, no keystroke has happened yet, or the
/// most recent keystroke is older than the fade window.
fn current_flash(engine: &crate::engine::Engine) -> Option<FlashFrame> {
    if !engine.flash_enabled() {
        return None;
    }
    let flash = engine.last_flash()?;
    let age_ms = flash.at.elapsed().as_millis() as u64;
    if age_ms > FLASH_FADE_MS {
        return None;
    }
    Some(FlashFrame {
        char: flash.char,
        age_ms,
    })
}

/// Render placements centered within `area`. `display` provides
/// the highlight target; `overlay` decides per-key colors; `flash`
/// (when `Some`) bumps the border of a single key to signal
/// last-keystroke feedback.
pub fn render_keyboard(
    f: &mut Frame,
    area: Rect,
    placements: &[Placement],
    display: &DisplayState,
    overlay: &dyn crate::renderer::overlay::KeyOverlay,
    flash: Option<&FlashFrame>,
) {
    if area.width < 3 || area.height < 3 || placements.is_empty() {
        return;
    }

    // Compute each key's left/right/top/bottom *edges* in terminal
    // cells, using floor on both sides. Rounding centers instead
    // would bias half-positive and half-negative fractions in
    // opposite directions and open 1-cell gaps at zero crossings
    // (e.g. keys at c=-0.5 and c=0.5 must tile edge-to-edge, but
    // round() puts them 1 cell apart).
    let edge = |v: f32, cell: u16| (v * cell as f32).floor() as i32;

    let mut min_col = i32::MAX;
    let mut max_col = i32::MIN;
    let mut min_row = i32::MAX;
    let mut max_row = i32::MIN;
    for p in placements {
        let left = edge(p.pos_a - p.width / 2.0, CELL_W);
        let right = edge(p.pos_a + p.width / 2.0, CELL_W);
        let top = edge(p.pos_b - p.height / 2.0, CELL_H);
        let bottom = edge(p.pos_b + p.height / 2.0, CELL_H);
        min_col = min_col.min(left);
        max_col = max_col.max(right);
        min_row = min_row.min(top);
        max_row = max_row.max(bottom);
    }
    let widget_w = (max_col - min_col).max(0) as u16;
    let widget_h = (max_row - min_row).max(0) as u16;

    let origin_x = area.x + area.width.saturating_sub(widget_w) / 2;
    let origin_y = area.y + area.height.saturating_sub(widget_h) / 2;

    let highlight_lower = display.highlight_char.map(|c| c.to_ascii_lowercase());

    for placement in placements {
        let left = edge(placement.pos_a - placement.width / 2.0, CELL_W);
        let right = edge(placement.pos_a + placement.width / 2.0, CELL_W);
        let top = edge(placement.pos_b - placement.height / 2.0, CELL_H);
        let bottom = edge(placement.pos_b + placement.height / 2.0, CELL_H);

        let x = origin_x as i32 + left - min_col;
        let y = origin_y as i32 + top - min_row;
        if x < 0 || y < 0 {
            continue;
        }
        let x = x as u16;
        let y = y as u16;
        let w = ((right - left).max(3)) as u16;
        let h = ((bottom - top).max(3)) as u16;
        if x + w > area.x + area.width || y + h > area.y + area.height {
            continue;
        }
        let rect = Rect::new(x, y, w, h);

        // Highlight matching: only typable keys can be highlighted
        // as typing targets. Named actions (shift/tab/etc.) whose
        // labels happen to start with the target char shouldn't
        // flash up when the user is meant to type a letter.
        let is_highlighted = match highlight_lower {
            Some(h) => placement.typable && placement.label.starts_with(h),
            None => false,
        };

        // Flash matching: same rules — typable keys whose label
        // starts with the flashed char. The flash renders
        // independent of the typing-target highlight, so a key can
        // be both "type this next" *and* "you just pressed it."
        let key_flash = flash.filter(|fl| {
            placement.typable && placement.label.starts_with(fl.char)
        });

        draw::draw_key(f, rect, placement, is_highlighted, overlay, key_flash);
    }
}
