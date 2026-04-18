//! Typing engine: char-by-char matching with WPM and accuracy tracking.
//!
//! Manages a word buffer, tracks cursor position, and calculates stats.
//! Used by word and text modes — knows nothing about rendering or input events.

use crate::stats::{Stats, StatsTracker};
use crate::words::random_word;
use std::time::Instant;

pub struct TypingTest {
    pub(crate) words: Vec<String>,
    /// None = endless mode
    pub(crate) target_count: Option<usize>,
    input: String,
    pub(crate) word_index: usize,
    pub(crate) char_index: usize,
    /// Are we waiting for a space between words?
    pub(crate) needs_space: bool,
    start_time: Option<Instant>,
    end_time: Option<Instant>,
}

impl TypingTest {
    pub fn new(target_count: Option<usize>, stats: &Stats) -> Self {
        let initial = target_count.map(|t| 10.min(t)).unwrap_or(10);
        let words: Vec<String> = (0..initial).map(|_| random_word(stats)).collect();
        TypingTest {
            words,
            target_count,
            input: String::new(),
            word_index: 0,
            char_index: 0,
            needs_space: false,
            start_time: None,
            end_time: None,
        }
    }

    /// Ensure we always have a few words buffered ahead of the current position.
    fn ensure_buffer(&mut self, stats: &Stats) {
        let max = self.target_count.unwrap_or(usize::MAX);
        while self.words.len() < self.word_index + 8 && self.words.len() < max {
            self.words.push(random_word(stats));
        }
    }

    pub fn current_word(&self) -> Option<&str> {
        self.words.get(self.word_index).map(|s| s.as_str())
    }

    pub fn expected_char(&self) -> Option<char> {
        if self.needs_space {
            Some(' ')
        } else {
            self.current_word()
                .and_then(|w| w.chars().nth(self.char_index))
        }
    }

    pub fn is_finished(&self) -> bool {
        match self.target_count {
            Some(target) => self.word_index >= target,
            None => false,
        }
    }

    /// Process a typed character.
    pub fn handle_input(&mut self, ch: char, stats: &mut StatsTracker) {
        if self.is_finished() {
            return;
        }
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        }

        if self.needs_space {
            let correct = ch == ' ';
            stats.record(' ', correct);
            if correct {
                self.needs_space = false;
                self.word_index += 1;
                self.char_index = 0;
                self.input.clear();
                self.ensure_buffer(stats.persistent());

                if self.is_finished() {
                    self.end_time = Some(Instant::now());
                }
            }
            return;
        }

        if let Some(expected) = self.expected_char() {
            let correct = ch == expected;
            stats.record(expected, correct);
            if correct {
                self.input.push(ch);
                self.char_index += 1;

                // Check if word is complete
                if let Some(word) = self.current_word()
                    && self.char_index >= word.len()
                {
                    // Last word doesn't need a space (only in non-endless mode)
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
        }
    }

    /// Words per minute for the active session.
    /// `stats` should be the session-scoped layer.
    pub fn wpm(&self, stats: &crate::stats::Stats) -> f64 {
        let elapsed = match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => end.duration_since(start),
            (Some(start), None) => start.elapsed(),
            _ => return 0.0,
        };
        let minutes = elapsed.as_secs_f64() / 60.0;
        if minutes < 0.01 {
            return 0.0;
        }
        // Standard: 5 chars = 1 word
        (stats.total_attempts() as f64 / 5.0) / minutes
    }
}
