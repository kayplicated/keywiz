//! Words exercise — type N randomly-picked words, or practice
//! endlessly. The word count is an instance-axis parameter
//! (`Alt+←/→`); `count = 0` means endless.

use std::time::Instant;

use crate::engine::placement::{DisplayState, WordsChar, WordsCharStatus, WordsDisplay};
use crate::exercise::Exercise;
use crate::words::random_word;

/// Core cursor logic shared by finite and endless word exercises.
struct WordsCore {
    words: Vec<String>,
    word_index: usize,
    char_index: usize,
    /// Waiting for a space between words.
    needs_space: bool,
    target_count: Option<usize>,
    start_time: Option<Instant>,
    end_time: Option<Instant>,
}

impl WordsCore {
    fn new(target_count: Option<usize>) -> Self {
        let initial = target_count.map(|t| 10.min(t)).unwrap_or(10);
        // Random words are picked without heat-weighting here since
        // the exercise doesn't own stats. Good enough for v1;
        // heat-weighting can come back when the engine wires stats
        // through a picker callback.
        let words: Vec<String> = (0..initial).map(|_| random_word_unweighted()).collect();
        WordsCore {
            words,
            word_index: 0,
            char_index: 0,
            needs_space: false,
            target_count,
            start_time: None,
            end_time: None,
        }
    }

    fn is_finished(&self) -> bool {
        match self.target_count {
            Some(target) => self.word_index >= target,
            None => false,
        }
    }

    fn current_word(&self) -> Option<&str> {
        self.words.get(self.word_index).map(|s| s.as_str())
    }

    fn expected(&self) -> Option<char> {
        if self.is_finished() {
            return None;
        }
        if self.needs_space {
            return Some(' ');
        }
        self.current_word()
            .and_then(|w| w.chars().nth(self.char_index))
    }

    fn advance(&mut self) {
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        }
        if self.is_finished() {
            return;
        }
        if self.needs_space {
            self.needs_space = false;
            self.word_index += 1;
            self.char_index = 0;
            self.ensure_buffer();
            if self.is_finished() {
                self.end_time = Some(Instant::now());
            }
            return;
        }
        self.char_index += 1;
        if let Some(word) = self.current_word()
            && self.char_index >= word.len()
        {
            if let Some(target) = self.target_count
                && self.word_index + 1 >= target
            {
                self.word_index += 1;
                self.end_time = Some(Instant::now());
                return;
            }
            self.needs_space = true;
        }
    }

    fn ensure_buffer(&mut self) {
        let max = self.target_count.unwrap_or(usize::MAX);
        while self.words.len() < self.word_index + 8 && self.words.len() < max {
            self.words.push(random_word_unweighted());
        }
    }

    /// Build the flat char list + cursor pos for the renderer.
    fn to_display(&self) -> WordsDisplay {
        let mut chars: Vec<WordsChar> = Vec::new();
        let mut cursor = 0;

        for (wi, word) in self.words.iter().enumerate() {
            if wi > 0 {
                let status = if wi == self.word_index + 1 && self.needs_space {
                    cursor = chars.len();
                    WordsCharStatus::Cursor
                } else {
                    WordsCharStatus::Separator
                };
                chars.push(WordsChar {
                    ch: '·',
                    status,
                });
            }

            for (ci, ch) in word.chars().enumerate() {
                let status = if wi < self.word_index {
                    WordsCharStatus::CompletedWord
                } else if wi == self.word_index && !self.needs_space {
                    if ci < self.char_index {
                        WordsCharStatus::Done
                    } else if ci == self.char_index {
                        cursor = chars.len();
                        WordsCharStatus::Cursor
                    } else {
                        WordsCharStatus::Pending
                    }
                } else {
                    WordsCharStatus::Pending
                };
                chars.push(WordsChar { ch, status });
            }
        }

        WordsDisplay {
            chars,
            cursor,
            word_index: self.word_index,
            target_count: self.target_count,
            is_finished: self.is_finished(),
        }
    }
}

fn random_word_unweighted() -> String {
    let empty = std::collections::HashMap::new();
    random_word(&empty)
}

pub struct WordsExercise(WordsCore);

impl WordsExercise {
    /// Build a words exercise with `count` target words. `count = 0`
    /// means endless practice with no finish condition.
    pub fn new(count: u32) -> Self {
        let target = if count == 0 { None } else { Some(count as usize) };
        WordsExercise(WordsCore::new(target))
    }
}

impl Exercise for WordsExercise {
    fn name(&self) -> &str {
        "words"
    }

    fn short(&self) -> &str {
        "Words"
    }

    fn expected(&self) -> Option<char> {
        self.0.expected()
    }

    fn advance(&mut self, _heat: &crate::exercise::HeatSteps, correct: bool) {
        if correct {
            self.0.advance()
        }
    }

    fn is_done(&self) -> bool {
        self.0.is_finished()
    }

    fn fill_display(&self, display: &mut DisplayState) {
        display.words = Some(self.0.to_display());
        display.highlight_char = self.0.expected();
    }
}
