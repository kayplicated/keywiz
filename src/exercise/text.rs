//! Passage-typing exercise — type through multi-line text loaded
//! from files in `texts/`. Arrow Left / Right switches passages.

use crossterm::event::{KeyCode, KeyEvent};
use std::time::Instant;

use crate::engine::placement::{DisplayState, TextDisplay};
use crate::exercise::Exercise;

struct Passage {
    title: String,
    body: String,
}

pub struct TextExercise {
    passages: Vec<Passage>,
    current: usize,
    chars: Vec<char>,
    cursor: usize,
    start_time: Option<Instant>,
    end_time: Option<Instant>,
}

impl TextExercise {
    pub fn new() -> Self {
        let passages = load_passages("texts");
        let chars = passages
            .first()
            .map(|p| p.body.chars().collect())
            .unwrap_or_default();
        TextExercise {
            passages,
            current: 0,
            chars,
            cursor: 0,
            start_time: None,
            end_time: None,
        }
    }

    fn switch_passage(&mut self, index: usize) {
        if self.passages.is_empty() {
            return;
        }
        self.current = index % self.passages.len();
        self.chars = self.passages[self.current].body.chars().collect();
        self.cursor = 0;
        self.start_time = None;
        self.end_time = None;
    }
}

impl Exercise for TextExercise {
    fn name(&self) -> &str {
        "text"
    }

    fn short(&self) -> &str {
        "Text"
    }

    fn expected(&self) -> Option<char> {
        self.chars.get(self.cursor).copied()
    }

    fn advance(&mut self) {
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        }
        self.cursor += 1;
        if self.cursor >= self.chars.len() {
            self.end_time = Some(Instant::now());
        }
    }

    fn is_done(&self) -> bool {
        !self.chars.is_empty() && self.cursor >= self.chars.len()
    }

    fn fill_display(&self, display: &mut DisplayState) {
        display.highlight_char = self.expected();
        if self.passages.is_empty() {
            return;
        }
        let passage = &self.passages[self.current];
        display.text = Some(TextDisplay {
            title: passage.title.clone(),
            passage_index: self.current,
            passage_total: self.passages.len(),
            body: passage.body.clone(),
            cursor: self.cursor,
            is_finished: self.is_done(),
        });
    }

    fn handle_control(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Left => {
                if self.passages.len() > 1 {
                    let idx = if self.current == 0 {
                        self.passages.len() - 1
                    } else {
                        self.current - 1
                    };
                    self.switch_passage(idx);
                }
                true
            }
            KeyCode::Right => {
                if self.passages.len() > 1 {
                    self.switch_passage(self.current + 1);
                }
                true
            }
            _ => false,
        }
    }
}

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
