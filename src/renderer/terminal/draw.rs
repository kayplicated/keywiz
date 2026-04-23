//! Drawing routines for the terminal renderer.
//!
//! Takes a `Placement` (already resolved by the engine) and paints
//! a box + label in ratatui. The active overlay decides which
//! surfaces (border / label / fill) get colored; the renderer
//! supplies sensible fallbacks and stacks the current-key
//! highlight on top so the user always sees where to type.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::engine::placement::Placement;
use crate::renderer::overlay::{KeyOverlay, KeyPaint};

use super::naming;
use super::FlashFrame;

/// Default color for any key surface no overlay paints —
/// mapped or not. "None overlay" reads identically to "unmapped
/// key": dim gray border + dim gray label (or no label). The user
/// sees a quiet baseline; overlays opt in to louder signals.
const DEFAULT_GRAY: Color = Color::DarkGray;

/// Paint one key in the given rect. `flash` adds a short-lived
/// bright border on top of whatever the overlay paints — `None`
/// keeps the key fully overlay-driven.
pub fn draw_key(
    f: &mut Frame,
    rect: Rect,
    placement: &Placement,
    is_highlighted: bool,
    overlay: &dyn KeyOverlay,
    flash: Option<&FlashFrame>,
) {
    if placement.label.is_empty() {
        // Unmapped key: dim outline, no label, no overlay input.
        let style = Style::default().fg(DEFAULT_GRAY);
        f.render_widget(
            Paragraph::new(box_lines(rect.width, rect.height, "", style)),
            rect,
        );
        return;
    }

    let label = label_for(placement);
    let mut paint = overlay.paint(placement);
    if let Some(flash) = flash {
        // Flash override: replace whatever the overlay decided for
        // the border with a bright color whose intensity steps
        // down as the keystroke ages. Label and fill stay as the
        // overlay chose so heat tints / finger colors don't
        // disappear during the flash.
        paint.border = Some(flash_color(flash.age_ms));
    }

    if is_highlighted {
        // Highlight composes on top of the overlay: the border color
        // comes from the overlay (so heat stays visible through the
        // highlight), but the box chars are heavy + bold; the label
        // is always white + bold so "type this" is unambiguous
        // regardless of overlay.
        let border_color = paint.border.unwrap_or(DEFAULT_GRAY);
        let border = Style::default().fg(border_color).bold();
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
        let style = resolve_style(&paint);
        f.render_widget(
            Paragraph::new(box_lines(rect.width, rect.height, &label, style)),
            rect,
        );
    }
}

/// Fold the overlay's paint into a single ratatui `Style`.
///
/// Terminal today renders the entire key (border and label) in one
/// `Style` — `box_lines` paints every cell with the same foreground.
/// When the overlay paints only the border (or only the label) we
/// pick the border color since it's the dominant visual. Whatever
/// is left unpainted falls back to [`DEFAULT_GRAY`], matching the
/// unmapped-key look. A future pass can split border/label into
/// separate styles; gui renderers already get that for free.
fn resolve_style(paint: &KeyPaint) -> Style {
    let fg = paint.border.or(paint.label).unwrap_or(DEFAULT_GRAY);
    let mut style = Style::default().fg(fg);
    if let Some(bg) = paint.fill {
        style = style.bg(bg);
    }
    if let Some(m) = paint.modifier {
        style = style.add_modifier(m);
    } else {
        // Avoid unused-import warning — `Modifier` is imported for
        // the signature even when no overlay asks for it.
        let _ = Modifier::empty();
    }
    style
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

/// Pick a flash border color based on keystroke age. Stepwise
/// fade from brightest (just pressed) to subtle (about to
/// expire), so the user perceives "that happened, and that's
/// fading now" without needing smooth RGB interpolation that
/// terminals don't support reliably.
fn flash_color(age_ms: u64) -> Color {
    if age_ms < 80 {
        Color::White
    } else if age_ms < 160 {
        Color::LightYellow
    } else {
        Color::Cyan
    }
}

fn truncate_to(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        chars.into_iter().take(max).collect()
    }
}
