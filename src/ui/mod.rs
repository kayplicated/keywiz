pub mod keyboard;

use ratatui::Frame;

use crate::app::App;
use crate::drill::Drill;
use crate::typing::TypingTest;

pub fn render_mode_select(f: &mut Frame, app: &App) {
    use ratatui::layout::{Constraint, Layout, Alignment};
    use ratatui::style::{Color, Style, Stylize};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    let area = f.area();

    // Center content vertically: 8 lines of menu content
    let [_, center, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(10),
        Constraint::Fill(1),
    ]).areas(area);

    let layout_name = &app.layout.name;
    let menu = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("key", Style::default().fg(Color::Cyan).bold()),
            Span::styled("wiz", Style::default().fg(Color::Yellow).bold()),
        ]),
        Line::from(""),
        Line::from(format!("Layout: {layout_name}")).style(Style::default().fg(Color::DarkGray)),
        Line::from(""),
        Line::from(vec![
            Span::styled("[1]", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" Key Drills — learn the layout"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("[2]", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" Typing Practice — type words"),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(
            Span::styled("ESC to quit", Style::default().fg(Color::DarkGray)),
        ),
    ])
    .alignment(Alignment::Center);
    f.render_widget(menu, center);
}

pub fn render_drill(f: &mut Frame, drill: &Drill, app: &App) {
    use ratatui::layout::{Constraint, Layout, Alignment};
    use ratatui::style::{Color, Style, Stylize};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    let area = f.area();

    // Content block: header(1) + gap(1) + prompt(3) + gap(1) + keyboard(12) + gap(1) + stats(1) = 20
    let content_h: u16 = 20;
    let [_, center, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(content_h),
        Constraint::Fill(1),
    ]).areas(area);

    let [header_area, _, prompt_area, _, kb_area, _, stats_area] = Layout::vertical([
        Constraint::Length(1),  // header
        Constraint::Length(1),  // gap
        Constraint::Length(3),  // prompt
        Constraint::Length(1),  // gap
        Constraint::Length(12), // keyboard
        Constraint::Length(1),  // gap
        Constraint::Length(1),  // stats
    ]).areas(center);

    // Header
    let header = Paragraph::new(Line::from(vec![
        Span::styled("Drill", Style::default().fg(Color::Cyan).bold()),
        Span::raw(" — "),
        Span::styled(drill.level.label(), Style::default().fg(Color::Yellow)),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(header, header_area);

    // Prompt: show the character to type (big block)
    let ch = drill.current;
    let prompt = Paragraph::new(vec![
        Line::from(Span::styled(
            format!("┌─────┐"),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("│  ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{ch}"), Style::default().fg(Color::White).bold()),
            Span::styled("  │", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(Span::styled(
            format!("└─────┘"),
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .alignment(Alignment::Center);
    f.render_widget(prompt, prompt_area);

    // Keyboard
    keyboard::render_keyboard(f, kb_area, &app.layout, Some(drill.current));

    // Stats
    let stats = Paragraph::new(Line::from(vec![
        Span::styled(format!("Correct: {}", drill.correct), Style::default().fg(Color::Green)),
        Span::raw("  "),
        Span::styled(format!("Wrong: {}", drill.wrong), Style::default().fg(Color::Red)),
        Span::raw("  "),
        Span::styled(format!("Accuracy: {:.0}%", drill.accuracy()), Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::styled(format!("Streak: {}", drill.streak), Style::default().fg(Color::Cyan)),
        Span::raw("  "),
        Span::styled("ESC to go back", Style::default().fg(Color::DarkGray)),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(stats, stats_area);
}

pub fn render_typing(f: &mut Frame, test: &TypingTest, app: &App) {
    use ratatui::layout::{Constraint, Layout, Alignment};
    use ratatui::style::{Color, Style, Stylize};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    let area = f.area();

    // Content block: header(1) + gap(1) + words(3) + gap(1) + keyboard(12) + gap(1) + stats(1) = 20
    let content_h: u16 = 20;
    let [_, center, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(content_h),
        Constraint::Fill(1),
    ]).areas(area);

    let [header_area, _, words_area, _, kb_area, _, stats_area] = Layout::vertical([
        Constraint::Length(1),  // header
        Constraint::Length(1),  // gap
        Constraint::Length(3),  // words
        Constraint::Length(1),  // gap
        Constraint::Length(12), // keyboard
        Constraint::Length(1),  // gap
        Constraint::Length(1),  // stats
    ]).areas(center);

    // Header
    let header = Paragraph::new(Line::from(vec![
        Span::styled("Typing Practice", Style::default().fg(Color::Cyan).bold()),
        Span::raw(format!(" — {}/{}", test.word_index, test.words.len())),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(header, header_area);

    // Words display
    if test.is_finished() {
        let results = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled("Done!", Style::default().fg(Color::Green).bold())),
            Line::from(format!("WPM: {:.0}  Accuracy: {:.0}%", test.wpm(), test.accuracy())),
        ])
        .alignment(Alignment::Center);
        f.render_widget(results, words_area);
    } else {
        // Build a flat list of (char, style) for all words joined by spaces.
        // Then render a window centered on the cursor position.
        #[derive(Clone)]
        struct StyledChar {
            ch: char,
            style: Style,
        }

        let mut flat: Vec<StyledChar> = Vec::new();
        let mut cursor_pos: usize = 0;

        for (wi, word) in test.words.iter().enumerate() {
            if wi > 0 {
                // Space between words — if we just finished word wi-1 and need space, highlight it
                let space_style = if wi == test.word_index + 1 && test.needs_space {
                    cursor_pos = flat.len();
                    Style::default().fg(Color::White).bold().underlined()
                } else if wi <= test.word_index {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                flat.push(StyledChar { ch: '·', style: space_style });
            }

            for (ci, ch) in word.chars().enumerate() {
                let style = if wi < test.word_index {
                    Style::default().fg(Color::DarkGray)
                } else if wi == test.word_index && !test.needs_space {
                    if ci < test.char_index {
                        Style::default().fg(Color::Green).bold()
                    } else if ci == test.char_index {
                        cursor_pos = flat.len();
                        Style::default().fg(Color::White).bold().underlined()
                    } else {
                        Style::default().fg(Color::Gray).bold()
                    }
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                flat.push(StyledChar { ch, style });
            }
        }

        // Render a window of `width` chars with cursor pinned to center.
        let width = words_area.width as usize;
        let half = width / 2;

        let mut visible: Vec<Span> = Vec::new();

        // Left padding if cursor is near the start
        if cursor_pos < half {
            let pad = half - cursor_pos;
            visible.push(Span::raw(" ".repeat(pad)));
        }

        let start = cursor_pos.saturating_sub(half);
        let end = (start + width).min(flat.len());

        for sc in &flat[start..end] {
            visible.push(Span::styled(sc.ch.to_string(), sc.style));
        }

        let words_display = Paragraph::new(vec![
            Line::from(""),
            Line::from(visible),
        ]);
        f.render_widget(words_display, words_area);

        // Keyboard — highlight expected char
        keyboard::render_keyboard(f, kb_area, &app.layout, test.expected_char());
    }

    // Stats
    let stats = Paragraph::new(Line::from(vec![
        Span::styled(format!("WPM: {:.0}", test.wpm()), Style::default().fg(Color::Cyan)),
        Span::raw("  "),
        Span::styled(format!("Accuracy: {:.0}%", test.accuracy()), Style::default().fg(Color::Yellow)),
        Span::raw("  "),
        Span::styled("ESC to go back", Style::default().fg(Color::DarkGray)),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(stats, stats_area);
}
