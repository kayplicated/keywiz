//! Drill engine: single-character presentation with adaptive level progression.
//!
//! Tracks accuracy over a rolling window and automatically adjusts difficulty.
//! Used by the drill mode — knows nothing about rendering or input events.

use crate::config::{
    LEVEL_DOWN_THRESHOLD, LEVEL_UP_THRESHOLD, MIN_KEYS_BEFORE_LEVEL_CHANGE, WINDOW_SIZE,
};
use crate::stats::StatsTracker;
use rand::prelude::IndexedRandom;
use std::collections::VecDeque;

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

/// Abstract char source: the caller provides per-level char sets so the
/// engine stays layout-agnostic. Modes build this from [`AppContext`] so
/// it works for both the legacy layout and the new grid paths.
///
/// `AppContext` implements this trait, but modes can't pass `&ctx` and
/// `&mut ctx.stats` at the same time — use [`snapshot_sources`] to
/// pre-compute per-level chars before calling [`Drill::handle_input`].
pub trait CharSource {
    fn chars_for(&self, level: DrillLevel) -> Vec<char>;
}

/// A concrete char source holding pre-computed chars for each level.
/// Built once per tick so modes can hand the engine `&mut stats` without
/// fighting the borrow checker.
pub struct LevelChars {
    pub home_row: Vec<char>,
    pub home_and_top: Vec<char>,
    pub all_rows: Vec<char>,
}

impl LevelChars {
    pub fn from_source(source: &dyn CharSource) -> Self {
        LevelChars {
            home_row: source.chars_for(DrillLevel::HomeRow),
            home_and_top: source.chars_for(DrillLevel::HomeAndTop),
            all_rows: source.chars_for(DrillLevel::AllRows),
        }
    }
}

impl CharSource for LevelChars {
    fn chars_for(&self, level: DrillLevel) -> Vec<char> {
        match level {
            DrillLevel::HomeRow => self.home_row.clone(),
            DrillLevel::HomeAndTop => self.home_and_top.clone(),
            DrillLevel::AllRows => self.all_rows.clone(),
        }
    }
}

pub struct Drill {
    pub(crate) level: DrillLevel,
    chars: Vec<char>,
    pub(crate) current: char,
    pub(crate) streak: usize,
    pub(crate) best_streak: usize,
    /// Rolling window of recent results (true = correct, false = wrong)
    window: VecDeque<bool>,
    /// Keys typed at the current level (resets on level change)
    keys_at_level: usize,
    /// Set briefly when level changes, for UI feedback
    pub(crate) level_changed: Option<LevelChange>,
}

#[derive(Debug, Clone, Copy)]
pub enum LevelChange {
    Up,
    Down,
}

impl Drill {
    pub fn new(source: &dyn CharSource, level: DrillLevel) -> Self {
        let chars = source.chars_for(level);
        let current = *chars.choose(&mut rand::rng()).unwrap_or(&'a');
        Drill {
            level,
            chars,
            current,
            streak: 0,
            best_streak: 0,
            window: VecDeque::with_capacity(WINDOW_SIZE),
            keys_at_level: 0,
            level_changed: None,
        }
    }

    pub fn next_char(&mut self) {
        let prev = self.current;
        loop {
            self.current = *self.chars.choose(&mut rand::rng()).unwrap_or(&'a');
            if self.current != prev || self.chars.len() <= 1 {
                break;
            }
        }
    }

    fn window_accuracy(&self) -> f64 {
        if self.window.is_empty() {
            return 100.0;
        }
        let correct = self.window.iter().filter(|&&b| b).count();
        (correct as f64 / self.window.len() as f64) * 100.0
    }

    /// Process a typed character. Returns true if the character was correct.
    pub fn handle_input(
        &mut self,
        ch: char,
        source: &dyn CharSource,
        stats: &mut StatsTracker,
    ) -> bool {
        self.keys_at_level += 1;
        self.level_changed = None;

        let is_correct = ch == self.current;
        stats.record(self.current, is_correct);

        if is_correct {
            self.streak += 1;
            if self.streak > self.best_streak {
                self.best_streak = self.streak;
            }
            self.next_char();
        } else {
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
            if acc >= LEVEL_UP_THRESHOLD
                && let Some(next) = self.level.next()
            {
                self.level = next;
                self.chars = source.chars_for(self.level);
                self.keys_at_level = 0;
                self.window.clear();
                self.level_changed = Some(LevelChange::Up);
                self.next_char();
            } else if acc < LEVEL_DOWN_THRESHOLD
                && let Some(prev) = self.level.prev()
            {
                self.level = prev;
                self.chars = source.chars_for(self.level);
                self.keys_at_level = 0;
                self.window.clear();
                self.level_changed = Some(LevelChange::Down);
                self.next_char();
            }
        }

        is_correct
    }
}
