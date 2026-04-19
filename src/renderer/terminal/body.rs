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

    // Clamp the scrolling word display to a readable width on wide
    // terminals so the cursor stays near the visual center.
    const MAX_W: u16 = 80;
    let inner_w = area.width.min(MAX_W);
    let inner_x = area.x + area.width.saturating_sub(inner_w) / 2;
    let area = Rect::new(inner_x, area.y, inner_w, area.height);

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

    let chars: Vec<char> = text.body.chars().collect();
    if chars.is_empty() || area.width < 10 || area.height == 0 {
        return;
    }

    // Clamp body to a readable max width and center within the
    // allotted area. Wide terminals otherwise smear the passage
    // across the full screen.
    const MAX_W: u16 = 80;
    let inner_w = area.width.min(MAX_W);
    let inner_x = area.x + area.width.saturating_sub(inner_w) / 2;
    let area = Rect::new(inner_x, area.y, inner_w, area.height);

    // Word-wrap into lines of at most `area.width` characters,
    // tracking each char's index in the original body so we can
    // style by cursor position.
    let max_w = area.width as usize;
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
        if current.len() >= max_w {
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

    // Window lines around the cursor so the active line stays
    // roughly centered vertically within the body band.
    let cursor_line_idx = lines
        .iter()
        .position(|l| l.iter().any(|(_, i)| *i == text.cursor))
        .unwrap_or(0);
    let visible = (area.height as usize).max(1);
    let start = cursor_line_idx.saturating_sub(visible / 2);
    let end = (start + visible).min(lines.len());

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

    f.render_widget(Paragraph::new(rendered), area);
}
