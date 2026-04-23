//! Footer renderer for terminal modes.
//!
//! Page-level layout is now done with
//! [`crate::renderer::terminal::view::View`] — this module owns
//! only the footer paint logic, which is shared across views.
//!
//! The footer is three rows:
//!   1. Ctrl bindings — hardware axis (keyboard, layout).
//!   2. Alt bindings  — exercise axis (category, instance).
//!   3. Error reason  — blank unless something's broken.
//!
//! Two rows instead of one for the bindings because the MAX_COLUMN_WIDTH
//! cap on the view builder (90 cols) isn't wide enough to fit all four
//! bindings on a single line without truncating the instance label when
//! names get long.

use ratatui::layout::Rect;

use crate::engine::placement::DisplayState;

pub fn render_footer(f: &mut ratatui::Frame, area: Rect, display: &DisplayState) {
    use ratatui::layout::Alignment;
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    let dim = Style::default().fg(Color::DarkGray);
    let red = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
    let red_dim = Style::default().fg(Color::Red);
    let name = Style::default().fg(Color::Gray);

    let keyboard_name = match &display.broken_keyboard {
        Some(b) => Span::styled(b.name.clone(), red),
        None => Span::styled(display.keyboard_short.clone(), name),
    };
    let layout_name = match &display.broken_layout {
        Some(b) => Span::styled(b.name.clone(), red),
        None => Span::styled(display.layout_short.clone(), name),
    };

    // Exercise category + instance counter. Category text always
    // includes `(n/m)` — for drill it renders as `(—/—)` so the
    // user reads "this category has no sub-axis" instead of
    // wondering why the indicator looks different from others.
    let (inst_i, inst_total) = display.exercise_instance;
    let counter = if inst_total == 0 {
        "(—/—)".to_string()
    } else {
        format!("({inst_i}/{inst_total})")
    };
    let category_span = Span::styled(
        format!("{} {}", display.exercise_short, counter),
        name,
    );

    let sep = Span::styled("     ", dim);
    let dot = Span::styled(" · ", dim);

    let ctrl_line = Line::from(vec![
        Span::styled("Ctrl+↑↓", dim),
        dot.clone(),
        keyboard_name,
        sep.clone(),
        Span::styled("Ctrl+←→", dim),
        dot.clone(),
        layout_name,
    ]);

    let mut alt_spans = vec![
        Span::styled("Alt+↑↓", dim),
        dot.clone(),
        category_span,
    ];
    // Only show the instance binding when there's an instance to
    // select; drill has no sideways axis so the key-hint is omitted.
    if let Some(label) = &display.exercise_instance_label {
        alt_spans.push(sep);
        alt_spans.push(Span::styled("Alt+←→", dim));
        alt_spans.push(dot);
        alt_spans.push(Span::styled(label.clone(), name));
    }
    let alt_line = Line::from(alt_spans);

    let reason = display
        .broken_keyboard
        .as_ref()
        .map(|b| &b.reason)
        .or_else(|| display.broken_layout.as_ref().map(|b| &b.reason));

    let error_line = match reason {
        Some(reason) => Line::from(Span::styled(truncate_reason(reason, 120), red_dim)),
        None => Line::from(""),
    };

    f.render_widget(
        Paragraph::new(vec![ctrl_line, alt_line, error_line]).alignment(Alignment::Center),
        area,
    );
}

fn truncate_reason(reason: &str, max: usize) -> String {
    let trimmed = reason.find(':').map_or(reason, |i| &reason[i + 1..]).trim();
    if trimmed.chars().count() <= max {
        trimmed.to_string()
    } else {
        let mut s: String = trimmed.chars().take(max - 1).collect();
        s.push('…');
        s
    }
}
