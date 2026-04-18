//! Key drill mode — single character at a time with adaptive difficulty.

use crate::app::AppContext;
use crate::engine::drill::{Drill, DrillLevel};
use crate::ui;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Alignment;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::{ActiveMode, ModeResult};

pub struct DrillMode {
    drill: Drill,
}

impl DrillMode {
    pub fn new(ctx: &AppContext, level: DrillLevel) -> Self {
        DrillMode {
            drill: Drill::new(&ctx.layout, level),
        }
    }

    pub fn handle_input(&mut self, key: KeyEvent, ctx: &mut AppContext) -> ModeResult {
        match key.code {
            KeyCode::Esc => ModeResult::SwitchTo(ActiveMode::Select(super::select::SelectMode)),
            KeyCode::Char(ch) => {
                let ch = ctx.translate_input(ch);
                self.drill.handle_input(ch, &ctx.layout, &mut ctx.stats);
                ModeResult::Stay
            }
            _ => ModeResult::Stay,
        }
    }

    pub fn render(&self, f: &mut Frame, ctx: &AppContext) {
        let areas = ui::centered_content_layout(f.area(), 3);

        // Header
        let header = Paragraph::new(Line::from(vec![
            Span::styled("Drill", Style::default().fg(Color::Cyan).bold()),
            Span::raw(" — "),
            Span::styled(
                self.drill.level.label(),
                Style::default().fg(Color::Yellow),
            ),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(header, areas.header);

        // Prompt: show the character to type
        let ch = self.drill.current;
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
        f.render_widget(prompt, areas.body);

        // Keyboard
        if ctx.show_keyboard {
            ui::keyboard::render_keyboard(
                f,
                areas.keyboard,
                &ctx.layout,
                Some(self.drill.current),
                ctx.split,
            );
        }

        // Stats
        let kb_hint = if ctx.show_keyboard {
            "TAB hide keyboard"
        } else {
            "TAB show keyboard"
        };
        let stats = Paragraph::new(Line::from(vec![
            Span::styled(
                format!("Correct: {}", self.drill.correct),
                Style::default().fg(Color::Green),
            ),
            Span::raw("  "),
            Span::styled(
                format!("Wrong: {}", self.drill.wrong),
                Style::default().fg(Color::Red),
            ),
            Span::raw("  "),
            Span::styled(
                format!("Accuracy: {:.0}%", self.drill.accuracy()),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("  "),
            Span::styled(
                format!("Streak: {}", self.drill.streak),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw("  "),
            Span::styled(kb_hint, Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled("ESC to go back", Style::default().fg(Color::DarkGray)),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(stats, areas.stats);
    }
}
