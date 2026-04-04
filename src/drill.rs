use crate::layout::Layout;
use rand::prelude::IndexedRandom;
use std::collections::VecDeque;

const WINDOW_SIZE: usize = 20;
const LEVEL_UP_THRESHOLD: f64 = 90.0;
const LEVEL_DOWN_THRESHOLD: f64 = 70.0;
const MIN_KEYS_BEFORE_LEVEL_CHANGE: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrillLevel {
    HomeRow,
    HomeAndTop,
    AllRows,
}

impl DrillLevel {
    pub fn label(self) -> &'static str {
        match self {
            DrillLevel::HomeRow => "Home Row",
            DrillLevel::HomeAndTop => "Home + Top Row",
            DrillLevel::AllRows => "All Rows",
        }
    }

    fn next(self) -> Option<DrillLevel> {
        match self {
            DrillLevel::HomeRow => Some(DrillLevel::HomeAndTop),
            DrillLevel::HomeAndTop => Some(DrillLevel::AllRows),
            DrillLevel::AllRows => None,
        }
    }

    fn prev(self) -> Option<DrillLevel> {
        match self {
            DrillLevel::HomeRow => None,
            DrillLevel::HomeAndTop => Some(DrillLevel::HomeRow),
            DrillLevel::AllRows => Some(DrillLevel::HomeAndTop),
        }
    }
}

pub struct Drill {
    pub level: DrillLevel,
    pub chars: Vec<char>,
    pub current: char,
    pub total: usize,
    pub correct: usize,
    pub wrong: usize,
    pub streak: usize,
    pub best_streak: usize,
    /// Rolling window of recent results (true = correct, false = wrong)
    window: VecDeque<bool>,
    /// Keys typed at the current level (resets on level change)
    keys_at_level: usize,
    /// Set briefly when level changes, for UI feedback
    pub level_changed: Option<LevelChange>,
}

#[derive(Debug, Clone, Copy)]
pub enum LevelChange {
    Up,
    Down,
}

impl Drill {
    pub fn new(layout: &Layout, level: DrillLevel) -> Self {
        let chars = chars_for_level(layout, level);
        let current = *chars.choose(&mut rand::rng()).unwrap_or(&'a');
        Drill {
            level,
            chars,
            current,
            total: 0,
            correct: 0,
            wrong: 0,
            streak: 0,
            best_streak: 0,
            window: VecDeque::with_capacity(WINDOW_SIZE),
            keys_at_level: 0,
            level_changed: None,
        }
    }

    pub fn next_char(&mut self) {
        self.current = *self.chars.choose(&mut rand::rng()).unwrap_or(&'a');
    }

    fn window_accuracy(&self) -> f64 {
        if self.window.is_empty() {
            return 100.0;
        }
        let correct = self.window.iter().filter(|&&b| b).count();
        (correct as f64 / self.window.len() as f64) * 100.0
    }

    pub fn handle_input(&mut self, ch: char, layout: &Layout) {
        self.total += 1;
        self.keys_at_level += 1;
        self.level_changed = None;

        let is_correct = ch == self.current;

        if is_correct {
            self.correct += 1;
            self.streak += 1;
            if self.streak > self.best_streak {
                self.best_streak = self.streak;
            }
            self.next_char();
        } else {
            self.wrong += 1;
            self.streak = 0;
        }

        // Update rolling window
        if self.window.len() >= WINDOW_SIZE {
            self.window.pop_front();
        }
        self.window.push_back(is_correct);

        // Check for level changes after enough keys at current level
        if self.keys_at_level >= MIN_KEYS_BEFORE_LEVEL_CHANGE
            && self.window.len() >= WINDOW_SIZE
        {
            let acc = self.window_accuracy();
            if acc >= LEVEL_UP_THRESHOLD {
                if let Some(next) = self.level.next() {
                    self.level = next;
                    self.chars = chars_for_level(layout, self.level);
                    self.keys_at_level = 0;
                    self.window.clear();
                    self.level_changed = Some(LevelChange::Up);
                    self.next_char();
                }
            } else if acc < LEVEL_DOWN_THRESHOLD {
                if let Some(prev) = self.level.prev() {
                    self.level = prev;
                    self.chars = chars_for_level(layout, self.level);
                    self.keys_at_level = 0;
                    self.window.clear();
                    self.level_changed = Some(LevelChange::Down);
                    self.next_char();
                }
            }
        }
    }

    pub fn accuracy(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            (self.correct as f64 / self.total as f64) * 100.0
        }
    }
}

fn chars_for_level(layout: &Layout, level: DrillLevel) -> Vec<char> {
    match level {
        DrillLevel::HomeRow => layout.home_row_chars(),
        DrillLevel::HomeAndTop => {
            let mut c = layout.home_row_chars();
            c.extend(
                layout.rows[1]
                    .keys
                    .iter()
                    .map(|k| k.lower)
                    .filter(|c| c.is_alphabetic()),
            );
            c
        }
        DrillLevel::AllRows => layout.all_chars(),
    }
}
