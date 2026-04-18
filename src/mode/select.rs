//! Mode selection screen — the main menu.

use crate::app::AppContext;
use crate::engine::drill::DrillLevel;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Alignment, Constraint, Layout};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use super::{ActiveMode, ModeResult};

pub struct SelectMode;

impl SelectMode {
    pub fn handle_input(&self, key: KeyEvent, ctx: &mut AppContext) -> ModeResult {
        match key.code {
            KeyCode::Esc => ModeResult::Quit,
            KeyCode::Char('1') => {
                ctx.stats.new_session();
                let mode = super::drill::DrillMode::new(ctx, DrillLevel::HomeRow);
                ModeResult::SwitchTo(ActiveMode::Drill(mode))
            }
            KeyCode::Char('2') => {
                ctx.stats.new_session();
                let mode = super::words::WordsMode::new(Some(20), ctx);
                ModeResult::SwitchTo(ActiveMode::Words(mode))
            }
            KeyCode::Char('3') => {
                ctx.stats.new_session();
                let mode = super::words::WordsMode::new(None, ctx);
                ModeResult::SwitchTo(ActiveMode::Words(mode))
            }
            KeyCode::Char('4') => {
                ctx.stats.new_session();
                let mode = super::text::TextMode::new();
                ModeResult::SwitchTo(ActiveMode::Text(mode))
            }
            _ => ModeResult::Stay,
        }
    }

    pub fn render(&self, f: &mut Frame, ctx: &AppContext) {
        let area = f.area();

        let [_, center, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(14),
            Constraint::Fill(1),
        ])
        .areas(area);

        let layout_name = &ctx.layout.name;
        let menu = Paragraph::new(vec![
            Line::from(vec![
                Span::styled("key", Style::default().fg(Color::Cyan).bold()),
                Span::styled("wiz", Style::default().fg(Color::Yellow).bold()),
            ]),
            Line::from(""),
            Line::from(if ctx.translate.is_some() {
                format!("Layout: {layout_name} (translating from QWERTY)")
            } else {
                format!("Layout: {layout_name}")
            })
            .style(Style::default().fg(Color::DarkGray)),
            Line::from(""),
            Line::from(vec![
                Span::styled("[1]", Style::default().fg(Color::Cyan).bold()),
                Span::raw(" Key Drills — learn the layout"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("[2]", Style::default().fg(Color::Cyan).bold()),
                Span::raw(" Typing Practice — 20 words"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("[3]", Style::default().fg(Color::Cyan).bold()),
                Span::raw(" Endless Mode — keep going"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("[4]", Style::default().fg(Color::Cyan).bold()),
                Span::raw(" Text Practice — type real passages"),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "ESC to quit",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .alignment(Alignment::Center);
        f.render_widget(menu, center);
    }
}
