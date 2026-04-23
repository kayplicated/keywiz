//! Words exercise — endless practice on a wordlist picked from
//! `words/`. The wordlist is an instance-axis parameter
//! (`Alt+←/→`), same cycling shape as text passages.
//!
//! keywiz is a layout trainer, not a typing speedrunner, so there
//! is no "type N words" finish condition. Sessions end when the
//! user ends them — the stats page is where the numbers live, not
//! an end screen.

use std::time::Instant;

use crate::engine::placement::{DisplayState, WordsChar, WordsCharStatus, WordsDisplay};
use crate::exercise::Exercise;
use crate::words::random_word;

struct WordsCore {
    list_index: usize,
    words: Vec<String>,
    word_index: usize,
    char_index: usize,
    /// Waiting for a space between words.
    needs_space: bool,
    start_time: Option<Instant>,
}

impl WordsCore {
    fn new(list_index: usize) -> Self {
        // Random words are picked without heat-weighting here since
        // the exercise doesn't own stats. Good enough for v1;
        // heat-weighting can come back when the engine wires stats
        // through a picker callback.
        let words: Vec<String> = (0..10).map(|_| random_word_unweighted(list_index)).collect();
        WordsCore {
            list_index,
            words,
            word_index: 0,
            char_index: 0,
            needs_space: false,
            start_time: None,
        }
    }

    fn current_word(&self) -> Option<&str> {
        self.words.get(self.word_index).map(|s| s.as_str())
    }

    fn expected(&self) -> Option<char> {
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
        if self.needs_space {
            self.needs_space = false;
            self.word_index += 1;
            self.char_index = 0;
            self.ensure_buffer();
            return;
        }
        self.char_index += 1;
        if let Some(word) = self.current_word()
            && self.char_index >= word.len()
        {
            self.needs_space = true;
        }
    }

    fn ensure_buffer(&mut self) {
        while self.words.len() < self.word_index + 8 {
            self.words.push(random_word_unweighted(self.list_index));
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
            is_finished: false,
        }
    }
}

fn random_word_unweighted(list_index: usize) -> String {
    let empty = std::collections::HashMap::new();
    random_word(list_index, &empty)
}

pub struct WordsExercise(WordsCore);

impl WordsExercise {
    /// Build a words exercise on wordlist `list_index`. Out-of-range
    /// indices clamp to the first list inside `random_word`.
    pub fn new(list_index: usize) -> Self {
        WordsExercise(WordsCore::new(list_index))
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
        false
    }

    fn fill_display(&self, display: &mut DisplayState) {
        display.words = Some(self.0.to_display());
        display.highlight_char = self.0.expected();
    }
}
