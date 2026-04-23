//! Passage-typing exercise — type through multi-line text loaded
//! from files in `texts/`. Passage switching is engine-level
//! (Alt+←/→), so this exercise is built at a specific passage index
//! and doesn't own the sideways navigation itself.

use std::sync::OnceLock;
use std::time::Instant;

use crate::engine::placement::{DisplayState, TextDisplay};
use crate::exercise::Exercise;

struct Passage {
    title: String,
    body: String,
}

/// Cached passage list. Loaded on first use; stable thereafter
/// within a single run so the engine's instance-count bound
/// matches what `TextExercise::new` will find.
static PASSAGES: OnceLock<Vec<Passage>> = OnceLock::new();

fn passages() -> &'static [Passage] {
    PASSAGES.get_or_init(|| load_passages("texts"))
}

pub struct TextExercise {
    current: usize,
    chars: Vec<char>,
    cursor: usize,
    #[allow(dead_code)]
    start_time: Option<Instant>,
    #[allow(dead_code)]
    end_time: Option<Instant>,
}

impl TextExercise {
    /// Build a text exercise positioned at `passage_index`. Out-of-
    /// range indices clamp to the first passage (or yield an empty
    /// exercise when `texts/` is empty).
    pub fn new(passage_index: usize) -> Self {
        let all = passages();
        let current = if all.is_empty() {
            0
        } else {
            passage_index.min(all.len() - 1)
        };
        let chars = all
            .get(current)
            .map(|p| p.body.chars().collect())
            .unwrap_or_default();
        TextExercise {
            current,
            chars,
            cursor: 0,
            start_time: None,
            end_time: None,
        }
    }

    /// Number of passages available on disk.
    pub fn passage_count() -> usize {
        passages().len()
    }

    /// Title of the passage at `index`, if any. Used by the footer
    /// to show which passage the user is currently on.
    pub fn passage_title(index: usize) -> Option<String> {
        passages().get(index).map(|p| p.title.clone())
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

    fn advance(&mut self, _heat: &crate::exercise::HeatSteps, correct: bool) {
        if !correct {
            return;
        }
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
        let all = passages();
        if all.is_empty() {
            return;
        }
        let passage = &all[self.current];
        display.text = Some(TextDisplay {
            title: passage.title.clone(),
            body: passage.body.clone(),
            cursor: self.cursor,
            is_finished: self.is_done(),
        });
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
