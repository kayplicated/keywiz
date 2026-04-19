//! Exercise-specific body rendering.
//!
//! Each exercise fills different fields of `DisplayState`. The
//! body renderer checks which ones are populated and draws the
//! appropriate UI — drill prompt, words scroll, text passage.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::engine::placement::{DisplayState, WordsCharStatus};

pub fn draw_body(f: &mut Frame, area: Rect, display: &DisplayState) {
    if let Some(ch) = display.drill_current_char {
        draw_drill_body(f, area, ch);
        return;
    }
    if let Some(words) = &display.words {
        draw_words_body(f, area, words);
        return;
    }
    if let Some(text) = &display.text {
        draw_text_body(f, area, text);
    }
}

fn draw_drill_body(f: &mut Frame, area: Rect, ch: char) {
    let prompt = Paragraph::new(vec![
        Line::from(Span::styled(
            "┌─────┐",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("│  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{ch}"),
                Style::default().fg(Color::White).bold(),
            ),
            Span::styled("  │", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(Span::styled(
            "└─────┘",
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .alignment(Alignment::Center);
    f.render_widget(prompt, area);
}

fn draw_words_body(
    f: &mut Frame,
    area: Rect,
    words: &crate::engine::placement::WordsDisplay,
) {
    if words.is_finished {
        let results = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Done!",
                Style::default().fg(Color::Green).bold(),
            )),
        ])
        .alignment(Alignment::Center);
        f.render_widget(results, area);
        return;
    }

    let width = area.width as usize;
    let half = width / 2;

    let mut visible: Vec<Span> = Vec::new();
    if words.cursor < half {
        visible.push(Span::raw(" ".repeat(half - words.cursor)));
    }

    let start = words.cursor.saturating_sub(half);
    let end = (start + width).min(words.chars.len());

    for c in &words.chars[start..end] {
        let style = match c.status {
            WordsCharStatus::Done => Style::default().fg(Color::Green).bold(),
            WordsCharStatus::Cursor => Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            WordsCharStatus::Pending => Style::default().fg(Color::Gray).bold(),
            WordsCharStatus::Separator => Style::default().fg(Color::DarkGray),
            WordsCharStatus::CompletedWord => Style::default().fg(Color::DarkGray),
        };
        visible.push(Span::styled(c.ch.to_string(), style));
    }

    let paragraph = Paragraph::new(vec![Line::from(""), Line::from(visible)]);
    f.render_widget(paragraph, area);
}

fn draw_text_body(
    f: &mut Frame,
    area: Rect,
    text: &crate::engine::placement::TextDisplay,
) {
    if text.is_finished {
        let results = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Done!",
                Style::default().fg(Color::Green).bold(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "◀ ▶ to switch passage, ESC to go back",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .alignment(Alignment::Center);
        f.render_widget(results, area);
        return;
    }

    // Lay out the body as wrapped lines. Minimal: just render
    // plain body with cursor highlighted.
    let chars: Vec<char> = text.body.chars().collect();
    let width = area.width as usize;
    if width < 4 || chars.is_empty() {
        return;
    }
    let inner_w = width.saturating_sub(2);

    // Word-wrap the body into lines.
    let mut lines: Vec<Vec<(char, usize)>> = Vec::new();
    let mut current: Vec<(char, usize)> = Vec::new();
    let mut last_space: Option<usize> = None;

    for (i, ch) in chars.iter().copied().enumerate() {
        if ch == '\n' {
            lines.push(std::mem::take(&mut current));
            last_space = None;
            continue;
        }
        if ch == ' ' {
            last_space = Some(current.len());
        }
        current.push((ch, i));
        if current.len() >= inner_w {
            if let Some(sp) = last_space {
                let rest = current.split_off(sp + 1);
                lines.push(std::mem::take(&mut current));
                current = rest;
                last_space = None;
            } else {
                lines.push(std::mem::take(&mut current));
                last_space = None;
            }
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }

    // Window the lines around the cursor.
    let cursor_line_idx = lines
        .iter()
        .position(|l| l.iter().any(|(_, i)| *i == text.cursor))
        .unwrap_or(0);
    let visible_line_count = (area.height as usize).max(1);
    let start = cursor_line_idx.saturating_sub(visible_line_count / 2);
    let end = (start + visible_line_count).min(lines.len());

    let dim = Style::default().fg(Color::DarkGray);
    let done = Style::default().fg(Color::Green);
    let cursor_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    let pending = Style::default().fg(Color::Gray);

    let rendered: Vec<Line> = lines[start..end]
        .iter()
        .map(|line| {
            let spans: Vec<Span> = line
                .iter()
                .map(|(ch, i)| {
                    let style = if *i < text.cursor {
                        done
                    } else if *i == text.cursor {
                        cursor_style
                    } else {
                        pending
                    };
                    Span::styled(ch.to_string(), style)
                })
                .collect();
            Line::from(spans)
        })
        .collect();

    // Header with title + counter
    let mut out: Vec<Line> = Vec::with_capacity(rendered.len() + 2);
    out.push(Line::from(vec![
        Span::styled(text.title.clone(), Style::default().fg(Color::Cyan).bold()),
        Span::raw(format!(
            "  ({}/{})",
            text.passage_index + 1,
            text.passage_total
        )),
        Span::styled("  ◀ ▶ switch", dim),
    ]));
    out.push(Line::from(""));
    out.extend(rendered);

    f.render_widget(Paragraph::new(out), area);
}
