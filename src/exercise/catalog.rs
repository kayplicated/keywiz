//! Exercise catalog and cycling.

use crate::exercise::drill::{DrillExercise, DrillLevel};
use crate::exercise::text::TextExercise;
use crate::exercise::words::{WordsEndless, WordsFinite};
use crate::exercise::Exercise;
use crate::keyboard::common::PhysicalKey;
use crate::keyboard::Keyboard;
use crate::mapping::{KeyMapping, Layout};

/// Shipped exercises in Alt+Arrow cycle order.
pub const EXERCISES: &[&str] = &[
    "drill-home",
    "drill-home-top",
    "drill-all",
    "words-20",
    "words-endless",
    "text",
];

/// Build an exercise by name against the active keyboard + layout.
pub fn build(name: &str, keyboard: &dyn Keyboard, layout: &Layout) -> Box<dyn Exercise> {
    match name {
        "drill-home" => Box::new(DrillExercise::new(
            "drill-home",
            "Drill: Home",
            DrillLevel::HomeRow,
            alpha_chars_at_rows(keyboard, layout, &[0]),
        )),
        "drill-home-top" => Box::new(DrillExercise::new(
            "drill-home-top",
            "Drill: Home+Top",
            DrillLevel::HomeAndTop,
            alpha_chars_at_rows(keyboard, layout, &[0, -1]),
        )),
        "drill-all" => Box::new(DrillExercise::new(
            "drill-all",
            "Drill: All",
            DrillLevel::AllRows,
            alpha_chars_at_rows(keyboard, layout, &[-2, -1, 0, 1]),
        )),
        "words-20" => Box::new(WordsFinite::new(20)),
        "words-endless" => Box::new(WordsEndless::new()),
        "text" => Box::new(TextExercise::new()),
        _ => Box::new(DrillExercise::new(
            "drill-home",
            "Drill: Home",
            DrillLevel::HomeRow,
            alpha_chars_at_rows(keyboard, layout, &[0]),
        )),
    }
}

pub fn next(current: &str) -> &'static str {
    let idx = EXERCISES.iter().position(|&n| n == current).unwrap_or(0);
    EXERCISES[(idx + 1) % EXERCISES.len()]
}

pub fn prev(current: &str) -> &'static str {
    let idx = EXERCISES.iter().position(|&n| n == current).unwrap_or(0);
    let p = if idx == 0 { EXERCISES.len() - 1 } else { idx - 1 };
    EXERCISES[p]
}

fn alpha_chars_at_rows(keyboard: &dyn Keyboard, layout: &Layout, rows: &[i32]) -> Vec<char> {
    keyboard
        .keys()
        .filter(|k: &&PhysicalKey| rows.contains(&k.r))
        .filter_map(|k| match layout.get(&k.id) {
            Some(KeyMapping::Char { lower, .. }) if lower.is_alphabetic() => Some(*lower),
            _ => None,
        })
        .collect()
}
