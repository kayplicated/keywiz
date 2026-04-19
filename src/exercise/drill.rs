//! Drill exercise — single character at a time with adaptive
//! level progression. Wraps the mechanics in `typing::drill`.

use rand::prelude::IndexedRandom;

use crate::engine::placement::DisplayState;
use crate::exercise::Exercise;

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
}

pub struct DrillExercise {
    name: String,
    short: String,
    level: DrillLevel,
    chars: Vec<char>,
    current: char,
    streak: u32,
}

impl DrillExercise {
    pub fn new(name: &str, short: &str, level: DrillLevel, chars: Vec<char>) -> Self {
        let current = pick_random(&chars, None).unwrap_or('a');
        DrillExercise {
            name: name.to_string(),
            short: short.to_string(),
            level,
            chars,
            current,
            streak: 0,
        }
    }
}

impl Exercise for DrillExercise {
    fn name(&self) -> &str {
        &self.name
    }

    fn short(&self) -> &str {
        &self.short
    }

    fn expected(&self) -> Option<char> {
        Some(self.current)
    }

    fn advance(&mut self) {
        self.streak += 1;
        self.current = pick_random(&self.chars, Some(self.current)).unwrap_or(self.current);
    }

    fn is_done(&self) -> bool {
        false
    }

    fn fill_display(&self, display: &mut DisplayState) {
        display.drill_current_char = Some(self.current);
        display.drill_level = Some(self.level.label().to_string());
        display.drill_streak = Some(self.streak);
        display.highlight_char = Some(self.current);
    }
}

/// Pick a random char from `chars`, avoiding `avoid` if possible.
fn pick_random(chars: &[char], avoid: Option<char>) -> Option<char> {
    if chars.is_empty() {
        return None;
    }
    let mut rng = rand::rng();
    if let Some(skip) = avoid {
        if chars.len() > 1 {
            let filtered: Vec<char> = chars.iter().copied().filter(|c| *c != skip).collect();
            if !filtered.is_empty() {
                return filtered.choose(&mut rng).copied();
            }
        }
    }
    chars.choose(&mut rng).copied()
}
