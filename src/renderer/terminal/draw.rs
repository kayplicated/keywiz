//! Ratatui drawing routines for the terminal renderer.
//!
//! Takes pre-computed placements from `placement.rs` and paints
//! boxes + labels for each key. Colors come from finger assignment
//! or heatmap depending on whether `heat` is supplied.

use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::keyboard::common::PhysicalKey;
use crate::mapping::KeyMapping;
use crate::stats::Stats;

use super::heatmap;
use super::naming::human_name;
use super::placement::TerminalPlacement;

/// Paint one key in the given rect.
pub fn draw_key(
    f: &mut Frame,
    rect: Rect,
    placement: &TerminalPlacement,
    mapping: Option<&KeyMapping>,
    is_highlighted: bool,
    heat: Option<&Stats>,
) {
    let Some(mapping) = mapping else {
        // Unmapped: dim outline.
        let style = Style::default().fg(Color::DarkGray);
        f.render_widget(Paragraph::new(box_lines(rect.width, rect.height, "", style)), rect);
        return;
    };

    let (label, lower_char) = label_for(mapping, placement.key);
    let color = color_for(placement.key, lower_char, heat);

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

fn label_for(mapping: &KeyMapping, key: &PhysicalKey) -> (String, Option<char>) {
    match mapping {
        KeyMapping::Char { lower, .. } => (lower.to_string(), Some(*lower)),
        KeyMapping::Named { name } => {
            // Named mappings get resolved through the id-naming
            // helper, which knows "shift" → "L-Shift" given the key's
            // id context. Fall back to the name itself if the helper
            // doesn't recognize it.
            let id_guess = format!("mods_{name}");
            let label = human_name(&id_guess);
            // If the helper returned the id verbatim (didn't match),
            // just use the name directly — nicer than "mods_whatever".
            let label = if label == id_guess {
                name.clone()
            } else {
                label
            };
            let _ = key;
            (label, None)
        }
    }
}

fn color_for(key: &PhysicalKey, lower_char: Option<char>, heat: Option<&Stats>) -> Color {
    match (heat, lower_char) {
        (Some(stats), Some(ch)) => heatmap::heat_color(stats, ch).unwrap_or(Color::DarkGray),
        _ => key.finger.color(),
    }
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
