//! Random word typing mode — finite (20 words) or endless.

use crate::app::AppContext;
use crate::engine::typing::TypingTest;
use crate::ui;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Alignment;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::{ActiveMode, ModeResult};

pub struct WordsMode {
    test: TypingTest,
}

impl WordsMode {
    /// Create a new words mode. Pass `Some(n)` for a fixed word count, `None` for endless.
    pub fn new(target_count: Option<usize>, ctx: &AppContext) -> Self {
        WordsMode {
            test: TypingTest::new(target_count, ctx.stats.persistent()),
        }
    }

    pub fn handle_input(&mut self, key: KeyEvent, ctx: &mut AppContext) -> ModeResult {
        match key.code {
            KeyCode::Esc => ModeResult::SwitchTo(ActiveMode::Select(super::select::SelectMode)),
            KeyCode::Char(ch) => {
                let ch = ctx.translate_input(ch);
                self.test.handle_input(ch, &mut ctx.stats);
                ModeResult::Stay
            }
            _ => ModeResult::Stay,
        }
    }

    pub fn render(&self, f: &mut Frame, ctx: &AppContext) {
        let areas = ui::centered_content_layout(f.area(), 3);

        // Header
        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                "Typing Practice",
                Style::default().fg(Color::Cyan).bold(),
            ),
            Span::raw(match self.test.target_count {
                Some(target) => format!(" — {}/{}", self.test.word_index, target),
                None => format!(" — {} words", self.test.word_index),
            }),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(header, areas.header);

        let session = ctx.stats.session();

        // Words display
        if self.test.is_finished() {
            let results = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Done!",
                    Style::default().fg(Color::Green).bold(),
                )),
                Line::from(format!(
                    "WPM: {:.0}  Accuracy: {:.0}%",
                    self.test.wpm(session),
                    session.overall_accuracy()
                )),
            ])
            .alignment(Alignment::Center);
            f.render_widget(results, areas.body);
        } else {
            self.render_words(f, &areas, ctx);
        }

        // Stats
        let kb_hint = if ctx.show_keyboard {
            "TAB hide keyboard"
        } else {
            "TAB show keyboard"
        };
        let stats = Paragraph::new(Line::from(vec![
            Span::styled(
                format!("WPM: {:.0}", self.test.wpm(session)),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw("  "),
            Span::styled(
                format!("Accuracy: {:.0}%", session.overall_accuracy()),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("  "),
            Span::styled(kb_hint, Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled("ESC to go back", Style::default().fg(Color::DarkGray)),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(stats, areas.stats);
    }

    fn render_words(&self, f: &mut Frame, areas: &ui::ContentAreas, ctx: &AppContext) {
        #[derive(Clone)]
        struct StyledChar {
            ch: char,
            style: Style,
        }

        let mut flat: Vec<StyledChar> = Vec::new();
        let mut cursor_pos: usize = 0;

        for (wi, word) in self.test.words.iter().enumerate() {
            if wi > 0 {
                let space_style = if wi == self.test.word_index + 1 && self.test.needs_space {
                    cursor_pos = flat.len();
                    Style::default().fg(Color::White).bold().underlined()
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                flat.push(StyledChar {
                    ch: '·',
                    style: space_style,
                });
            }

            for (ci, ch) in word.chars().enumerate() {
                let style = if wi < self.test.word_index {
                    Style::default().fg(Color::DarkGray)
                } else if wi == self.test.word_index && !self.test.needs_space {
                    if ci < self.test.char_index {
                        Style::default().fg(Color::Green).bold()
                    } else if ci == self.test.char_index {
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

        // Render a window of `width` chars with cursor pinned to center
        let width = areas.body.width as usize;
        let half = width / 2;

        let mut visible: Vec<Span> = Vec::new();

        if cursor_pos < half {
            let pad = half - cursor_pos;
            visible.push(Span::raw(" ".repeat(pad)));
        }

        let start = cursor_pos.saturating_sub(half);
        let end = (start + width).min(flat.len());

        for sc in &flat[start..end] {
            visible.push(Span::styled(sc.ch.to_string(), sc.style));
        }

        let words_display = Paragraph::new(vec![Line::from(""), Line::from(visible)]);
        f.render_widget(words_display, areas.body);

        // Keyboard — highlight expected char
        if ctx.show_keyboard {
            let heat = ctx.show_heatmap.then(|| ctx.stats.persistent());
            ui::keyboard::render_keyboard(
                f,
                areas.keyboard,
                &ctx.layout,
                self.test.expected_char(),
                ctx.split,
                heat,
            );
        }
    }
}
