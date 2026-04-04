use rand::prelude::IndexedRandom;
use std::time::Instant;

const WORDS: &str = include_str!("words.txt");

fn random_word() -> String {
    let all_words: Vec<&str> = WORDS.lines().filter(|l| !l.is_empty()).collect();
    all_words
        .choose(&mut rand::rng())
        .unwrap_or(&"the")
        .to_string()
}

pub struct TypingTest {
    pub words: Vec<String>,
    /// None = endless mode
    pub target_count: Option<usize>,
    pub input: String,
    pub word_index: usize,
    pub char_index: usize,
    /// Are we waiting for a space between words?
    pub needs_space: bool,
    pub correct: usize,
    pub wrong: usize,
    pub start_time: Option<Instant>,
    pub end_time: Option<Instant>,
}

impl TypingTest {
    pub fn new(target_count: Option<usize>) -> Self {
        let initial = target_count.map(|t| 10.min(t)).unwrap_or(10);
        let words: Vec<String> = (0..initial)
            .map(|_| random_word())
            .collect();
        TypingTest {
            words,
            target_count,
            input: String::new(),
            word_index: 0,
            char_index: 0,
            needs_space: false,
            correct: 0,
            wrong: 0,
            start_time: None,
            end_time: None,
        }
    }

    /// Ensure we always have a few words buffered ahead of the current position.
    fn ensure_buffer(&mut self) {
        let max = self.target_count.unwrap_or(usize::MAX);
        while self.words.len() < self.word_index + 8
            && self.words.len() < max
        {
            self.words.push(random_word());
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
            None => false, // endless mode never finishes
        }
    }

    pub fn handle_input(&mut self, ch: char) {
        if self.is_finished() {
            return;
        }
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        }

        if self.needs_space {
            if ch == ' ' {
                self.correct += 1;
                self.needs_space = false;
                self.word_index += 1;
                self.char_index = 0;
                self.input.clear();
                self.ensure_buffer();

                if self.is_finished() {
                    self.end_time = Some(Instant::now());
                }
            } else {
                self.wrong += 1;
            }
            return;
        }

        if let Some(expected) = self.expected_char() {
            if ch == expected {
                self.correct += 1;
                self.input.push(ch);
                self.char_index += 1;

                // Check if word is complete
                if let Some(word) = self.current_word() {
                    if self.char_index >= word.len() {
                        // Last word doesn't need a space (only in non-endless mode)
                        if let Some(target) = self.target_count {
                            if self.word_index + 1 >= target {
                                self.word_index += 1;
                                self.end_time = Some(Instant::now());
                                return;
                            }
                        }
                        self.needs_space = true;
                    }
                }
            } else {
                self.wrong += 1;
            }
        }
    }

    pub fn wpm(&self) -> f64 {
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
        let total_chars = self.correct + self.wrong;
        (total_chars as f64 / 5.0) / minutes
    }

    pub fn accuracy(&self) -> f64 {
        let total = self.correct + self.wrong;
        if total == 0 {
            100.0
        } else {
            (self.correct as f64 / total as f64) * 100.0
        }
    }
}
