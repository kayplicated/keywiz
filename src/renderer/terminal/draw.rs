//! Drawing routines for the terminal renderer.
//!
//! Takes a `Placement` (already resolved by the engine) and paints
//! a box + label in ratatui. Colors come from finger assignment or
//! heat, depending on the `heatmap_on` toggle from DisplayState.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::engine::placement::Placement;

use super::heatmap;
use super::naming;

/// Paint one key in the given rect.
pub fn draw_key(
    f: &mut Frame,
    rect: Rect,
    placement: &Placement,
    is_highlighted: bool,
    heatmap_on: bool,
) {
    if placement.label.is_empty() {
        // Unmapped key: dim outline, no label.
        let style = Style::default().fg(Color::DarkGray);
        f.render_widget(
            Paragraph::new(box_lines(rect.width, rect.height, "", style)),
            rect,
        );
        return;
    }

    let label = label_for(placement);
    let color = color_for(placement, heatmap_on);

    if is_highlighted {
        let border = Style::default().fg(color).bold();
        let letter = Style::default().fg(Color::White).bold();
        f.render_widget(
            Paragraph::new(box_lines_highlighted(
                rect.width,
                rect.height,
                &label,
                border,
                letter,
            )),
            rect,
        );
    } else {
        let style = Style::default().fg(color);
        f.render_widget(
            Paragraph::new(box_lines(rect.width, rect.height, &label, style)),
            rect,
        );
    }
}

/// Turn the engine-provided label into a display string. For typed
/// keys the label is already the single char. For named keys the
/// label is the raw action name; format it as a short terminal
/// label ("shift" → "Shift", "shift_left" → "L-Shift", etc.).
fn label_for(placement: &Placement) -> String {
    // Single-char labels are typed chars — display verbatim.
    if placement.label.chars().count() == 1 {
        return placement.label.clone();
    }
    // Multi-char labels are named actions. Prefix with "mods_" so
    // `naming::human_name` can use its mods lookup table; the
    // prefix is stripped inside the helper when applicable.
    let id_guess = format!("mods_{}", placement.label);
    let resolved = naming::human_name(&id_guess);
    if resolved == id_guess {
        // naming didn't recognize it — fall back to the raw name
        // title-cased a little.
        placement.label.clone()
    } else {
        resolved
    }
}

fn color_for(placement: &Placement, heatmap_on: bool) -> Color {
    if heatmap_on {
        if let Some(heat) = placement.heat {
            return heatmap::color_for_heat(heat);
        }
        return Color::DarkGray;
    }
    placement.finger.color()
}

fn box_lines(w: u16, h: u16, label: &str, style: Style) -> Vec<Line<'static>> {
    if w < 3 || h < 3 {
        return vec![Line::raw("")];
    }
    let inner_w = (w - 2) as usize;
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(h as usize);

    lines.push(Line::from(Span::styled(
        format!("┌{}┐", "─".repeat(inner_w)),
        style,
    )));

    let middle_idx = (h - 2) / 2;
    let label = truncate_to(label, inner_w);
    for i in 0..(h - 2) {
        if i == middle_idx {
            let label_w = label.chars().count();
            let left_pad = inner_w.saturating_sub(label_w) / 2;
            let right_pad = inner_w.saturating_sub(label_w) - left_pad;
            lines.push(Line::from(Span::styled(
                format!(
                    "│{}{}{}│",
                    " ".repeat(left_pad),
                    label,
                    " ".repeat(right_pad)
                ),
                style,
            )));
        } else {
            lines.push(Line::from(Span::styled(
                format!("│{}│", " ".repeat(inner_w)),
                style,
            )));
        }
    }

    lines.push(Line::from(Span::styled(
        format!("└{}┘", "─".repeat(inner_w)),
        style,
    )));
    lines
}

fn box_lines_highlighted(
    w: u16,
    h: u16,
    label: &str,
    border: Style,
    letter: Style,
) -> Vec<Line<'static>> {
    if w < 3 || h < 3 {
        return vec![Line::raw("")];
    }
    let inner_w = (w - 2) as usize;
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(h as usize);

    lines.push(Line::from(Span::styled(
        format!("╔{}╗", "═".repeat(inner_w)),
        border,
    )));

    let middle_idx = (h - 2) / 2;
    let label = truncate_to(label, inner_w);
    for i in 0..(h - 2) {
        if i == middle_idx {
            let label_w = label.chars().count();
            let left_pad = inner_w.saturating_sub(label_w) / 2;
            let right_pad = inner_w.saturating_sub(label_w) - left_pad;
            lines.push(Line::from(vec![
                Span::styled("║", border),
                Span::styled(" ".repeat(left_pad), letter),
                Span::styled(label.clone(), letter),
                Span::styled(" ".repeat(right_pad), letter),
                Span::styled("║", border),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                format!("║{}║", " ".repeat(inner_w)),
                border,
            )));
        }
    }

    lines.push(Line::from(Span::styled(
        format!("╚{}╝", "═".repeat(inner_w)),
        border,
    )));
    lines
}

fn truncate_to(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        chars.into_iter().take(max).collect()
    }
}
