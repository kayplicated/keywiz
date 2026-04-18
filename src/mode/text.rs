//! Passage typing mode — type through real text loaded from files.
//!
//! Passages are loaded from the `texts/` directory. Arrow left/right
//! switches between passages. The display shows multiple lines with
//! the current position highlighted.

use crate::app::AppContext;
use crate::ui;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Alignment;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use std::time::Instant;

use super::{ActiveMode, ModeResult};

/// A loaded passage with title and body text.
struct Passage {
    title: String,
    body: String,
}

/// Load all passages from a directory. Each file's first line is the title,
/// the rest is the body. Files are sorted by name for stable ordering.
fn load_passages(dir: &str) -> Vec<Passage> {
    let mut passages = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return passages;
    };
    let mut paths: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();

    for path in paths {
        if path.extension().is_some_and(|e| e == "txt")
            && let Ok(content) = std::fs::read_to_string(&path)
        {
            let mut lines = content.lines();
            let title = lines.next().unwrap_or("Untitled").to_string();
            let body = lines.collect::<Vec<_>>().join("\n").trim().to_string();
            if !body.is_empty() {
                passages.push(Passage { title, body });
            }
        }
    }
    passages
}

pub struct TextMode {
    passages: Vec<Passage>,
    current_passage: usize,
    /// Flattened characters of the current passage body.
    chars: Vec<char>,
    /// Current cursor position in chars.
    cursor: usize,
    start_time: Option<Instant>,
    end_time: Option<Instant>,
}

impl TextMode {
    pub fn new() -> Self {
        let passages = load_passages("texts");
        let chars = passages
            .first()
            .map(|p| p.body.chars().collect())
            .unwrap_or_default();
        TextMode {
            passages,
            current_passage: 0,
            chars,
            cursor: 0,
            start_time: None,
            end_time: None,
        }
    }

    fn switch_passage(&mut self, index: usize, ctx: &mut AppContext) {
        self.current_passage = index;
        self.chars = self.passages[index].body.chars().collect();
        self.cursor = 0;
        self.start_time = None;
        self.end_time = None;
        ctx.stats.new_session();
    }

    fn is_finished(&self) -> bool {
        self.cursor >= self.chars.len()
    }

    fn expected_char(&self) -> Option<char> {
        self.chars.get(self.cursor).copied()
    }

    fn wpm(&self, stats: &crate::stats::Stats) -> f64 {
        let elapsed = match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => end.duration_since(start),
            (Some(start), None) => start.elapsed(),
            _ => return 0.0,
        };
        let minutes = elapsed.as_secs_f64() / 60.0;
        if minutes < 0.01 {
            return 0.0;
        }
        (stats.total_attempts() as f64 / 5.0) / minutes
    }

    pub fn handle_input(&mut self, key: KeyEvent, ctx: &mut AppContext) -> ModeResult {
        if self.passages.is_empty() {
            if key.code == KeyCode::Esc {
                return ModeResult::SwitchTo(ActiveMode::Select(super::select::SelectMode));
            }
            return ModeResult::Stay;
        }

        match key.code {
            KeyCode::Esc => {
                ModeResult::SwitchTo(ActiveMode::Select(super::select::SelectMode))
            }
            KeyCode::Left => {
                if self.passages.len() > 1 {
                    let idx = if self.current_passage == 0 {
                        self.passages.len() - 1
                    } else {
                        self.current_passage - 1
                    };
                    self.switch_passage(idx, ctx);
                }
                ModeResult::Stay
            }
            KeyCode::Right => {
                if self.passages.len() > 1 {
                    let idx = (self.current_passage + 1) % self.passages.len();
                    self.switch_passage(idx, ctx);
                }
                ModeResult::Stay
            }
            KeyCode::Char(ch) => {
                if self.is_finished() {
                    return ModeResult::Stay;
                }
                if self.start_time.is_none() {
                    self.start_time = Some(Instant::now());
                }
                let ch = ctx.translate_input(ch);
                if let Some(expected) = self.expected_char() {
                    let correct = ch == expected;
                    ctx.stats.record(expected, correct);
                    if correct {
                        self.cursor += 1;
                        if self.is_finished() {
                            self.end_time = Some(Instant::now());
                        }
                    }
                }
                ModeResult::Stay
            }
            _ => ModeResult::Stay,
        }
    }

    pub fn render(&self, f: &mut Frame, ctx: &AppContext) {
        if self.passages.is_empty() {
            self.render_no_passages(f);
            return;
        }

        let areas = ui::centered_content_layout(f.area(), 10);
        let passage = &self.passages[self.current_passage];

        // Header: title + passage counter
        let header = Paragraph::new(Line::from(vec![
            Span::styled(&passage.title, Style::default().fg(Color::Cyan).bold()),
            Span::raw(format!(
                " — {}/{}",
                self.current_passage + 1,
                self.passages.len()
            )),
            Span::styled(
                "  ◀ ▶ switch",
                Style::default().fg(Color::DarkGray),
            ),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(header, areas.header);

        let session = ctx.stats.session();

        // Body: multi-line passage with cursor
        if self.is_finished() {
            let results = Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Done!",
                    Style::default().fg(Color::Green).bold(),
                )),
                Line::from(format!(
                    "WPM: {:.0}  Accuracy: {:.0}%",
                    self.wpm(session),
                    session.overall_accuracy()
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "◀ ▶ to switch passage, ESC to go back",
                    Style::default().fg(Color::DarkGray),
                )),
            ])
            .alignment(Alignment::Center);
            f.render_widget(results, areas.body);
        } else {
            self.render_passage(f, &areas, ctx);
        }

        // Stats
        let kb_hint = if ctx.show_keyboard {
            "TAB hide keyboard"
        } else {
            "TAB show keyboard"
        };
        let stats = Paragraph::new(Line::from(vec![
            Span::styled(
                format!("WPM: {:.0}", self.wpm(session)),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw("  "),
            Span::styled(
                format!("Accuracy: {:.0}%", session.overall_accuracy()),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("  "),
            Span::styled(
                format!(
                    "{}/{}",
                    self.cursor,
                    self.chars.len()
                ),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw("  "),
            Span::styled(kb_hint, Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled("ESC to go back", Style::default().fg(Color::DarkGray)),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(stats, areas.stats);
    }

    fn render_passage(&self, f: &mut Frame, areas: &ui::ContentAreas, ctx: &AppContext) {
        use ratatui::layout::{Constraint, Layout};

        // Constrain text to a readable width with horizontal padding
        let max_text_width: u16 = 72;
        let [_, text_area, _] = Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Max(max_text_width),
            Constraint::Fill(1),
        ])
        .areas(areas.body);

        let width = text_area.width as usize;
        let height = text_area.height as usize;
        if width == 0 || height == 0 {
            return;
        }

        // Word-wrap the passage into lines that fit the display width
        let wrapped = word_wrap(&self.chars, width);

        // Find which wrapped line the cursor is on
        let mut cursor_line = 0;
        let mut chars_before = 0;
        for (i, line) in wrapped.iter().enumerate() {
            if chars_before + line.len() > self.cursor {
                cursor_line = i;
                break;
            }
            chars_before += line.len();
            if i == wrapped.len() - 1 {
                cursor_line = i;
            }
        }

        // Window the visible lines around the cursor line
        let half = height / 2;
        let start_line = cursor_line.saturating_sub(half);
        let end_line = (start_line + height).min(wrapped.len());

        let mut lines: Vec<Line> = Vec::new();
        let mut char_offset = wrapped[..start_line].iter().map(|l| l.len()).sum::<usize>();

        for line_chars in &wrapped[start_line..end_line] {
            let mut spans: Vec<Span> = Vec::new();
            for &ch in line_chars {
                let style = if char_offset < self.cursor {
                    // Already typed
                    Style::default().fg(Color::DarkGray)
                } else if char_offset == self.cursor {
                    // Current position
                    Style::default().fg(Color::White).bold().underlined()
                } else {
                    // Upcoming
                    Style::default().fg(Color::Gray)
                };
                let display = if ch == '\n' { ' ' } else { ch };
                spans.push(Span::styled(display.to_string(), style));
                char_offset += 1;
            }
            lines.push(Line::from(spans));
        }

        f.render_widget(Paragraph::new(lines), text_area);

        // Keyboard
        if ctx.show_keyboard {
            let highlight = self.expected_char().filter(|c| *c != '\n');
            let heat = ctx.show_heatmap.then(|| ctx.stats.persistent());
            if let Some(mgr) = &ctx.grid_manager {
                ui::grid::render_grid(f, areas.keyboard, mgr.grid(), highlight, heat);
            } else {
                ui::keyboard::render_keyboard(
                    f,
                    areas.keyboard,
                    &ctx.layout,
                    highlight,
                    ctx.split,
                    heat,
                );
            }
        }
    }

    fn render_no_passages(&self, f: &mut Frame) {
        let area = f.area();
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "No passages found",
                Style::default().fg(Color::Red).bold(),
            )),
            Line::from(""),
            Line::from("Add .txt files to the texts/ directory."),
            Line::from("First line = title, rest = passage text."),
            Line::from(""),
            Line::from(Span::styled(
                "ESC to go back",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .alignment(Alignment::Center);
        f.render_widget(msg, area);
    }
}

/// Word-wrap characters into lines of at most `width` characters.
/// Preserves newlines from the original text. Each "line" in the output
/// is a slice of the character sequence including trailing spaces/newlines.
fn word_wrap(chars: &[char], width: usize) -> Vec<Vec<char>> {
    let mut lines: Vec<Vec<char>> = Vec::new();
    let mut current_line: Vec<char> = Vec::new();
    let mut col = 0;

    for &ch in chars {
        if ch == '\n' {
            current_line.push(ch);
            lines.push(std::mem::take(&mut current_line));
            col = 0;
            continue;
        }

        if col >= width {
            // Try to break at last space
            if let Some(space_pos) = current_line.iter().rposition(|&c| c == ' ') {
                let remainder: Vec<char> = current_line[space_pos + 1..].to_vec();
                current_line.truncate(space_pos + 1);
                lines.push(std::mem::take(&mut current_line));
                current_line = remainder;
                col = current_line.len();
            } else {
                lines.push(std::mem::take(&mut current_line));
                col = 0;
            }
        }

        current_line.push(ch);
        col += 1;
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}
