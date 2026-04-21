//! Exercise catalog and 2D cycling.
//!
//! Two axes:
//! - **Category** (`Alt+↑/↓`) — drill, words, text.
//! - **Instance** (`Alt+←/→`) — mode within a category. Drill has
//!   no instances; words has five lengths (`10/20/50/100/0` where
//!   `0` means endless); text has one instance per file in `texts/`.
//!
//! Both axes wrap at their ends. Switching category restores the
//! last-used instance within that category via engine-side memory
//! (not stored here — this module is stateless).

use crate::exercise::drill::DrillExercise;
use crate::exercise::text::TextExercise;
use crate::exercise::words::WordsExercise;
use crate::exercise::Exercise;
use crate::keyboard::common::PhysicalKey;
use crate::keyboard::Keyboard;
use crate::mapping::{KeyMapping, Layout};
use crate::stats::Stats;

/// Category axis. Order is the Alt+↑/↓ cycle order.
pub const CATEGORIES: &[&str] = &["drill", "words", "text"];

/// The five shipped word-count instances. `0` = endless.
pub const WORDS_INSTANCES: &[u32] = &[10, 20, 50, 100, 0];

/// Build an exercise for the active (category, instance) pair
/// against the current keyboard/layout/stats. Unknown category
/// falls through to drill.
pub fn build(
    category: &str,
    instance: usize,
    keyboard: &dyn Keyboard,
    layout: &Layout,
    stats: &Stats,
) -> Box<dyn Exercise> {
    match category {
        "words" => {
            let count = WORDS_INSTANCES
                .get(instance)
                .copied()
                .unwrap_or(WORDS_INSTANCES[0]);
            Box::new(WordsExercise::new(count))
        }
        "text" => Box::new(TextExercise::new(instance)),
        // "drill" and anything unknown.
        _ => Box::new(DrillExercise::new(
            drill_chars_by_level(keyboard, layout),
            stats,
        )),
    }
}

/// Number of instances in `category`. Drill has none (returns 0);
/// words has `WORDS_INSTANCES.len()`; text has one per file in
/// `texts/`. Engine uses this to bound instance cycling.
pub fn instance_count(category: &str) -> usize {
    match category {
        "drill" => 0,
        "words" => WORDS_INSTANCES.len(),
        "text" => TextExercise::passage_count(),
        _ => 0,
    }
}

/// Human label for the current instance in footer indicators, e.g.
/// `"50"`, `"Endless"`, `"The Commit"`. Returns `None` when the
/// category has no instance axis.
pub fn instance_label(category: &str, instance: usize) -> Option<String> {
    match category {
        "drill" => None,
        "words" => {
            let count = WORDS_INSTANCES.get(instance).copied()?;
            Some(if count == 0 {
                "Endless".to_string()
            } else {
                count.to_string()
            })
        }
        "text" => TextExercise::passage_title(instance),
        _ => None,
    }
}

/// Next category in cycle order (wraps).
pub fn next_category(current: &str) -> &'static str {
    let idx = CATEGORIES.iter().position(|&n| n == current).unwrap_or(0);
    CATEGORIES[(idx + 1) % CATEGORIES.len()]
}

/// Previous category in cycle order (wraps).
pub fn prev_category(current: &str) -> &'static str {
    let idx = CATEGORIES.iter().position(|&n| n == current).unwrap_or(0);
    let p = if idx == 0 {
        CATEGORIES.len() - 1
    } else {
        idx - 1
    };
    CATEGORIES[p]
}

/// Next instance within `category`, wrapping. `None` when the
/// category has no instances.
pub fn next_instance(category: &str, current: usize) -> Option<usize> {
    let n = instance_count(category);
    if n == 0 {
        return None;
    }
    Some((current + 1) % n)
}

/// Previous instance within `category`, wrapping. `None` when the
/// category has no instances.
pub fn prev_instance(category: &str, current: usize) -> Option<usize> {
    let n = instance_count(category);
    if n == 0 {
        return None;
    }
    Some(if current == 0 { n - 1 } else { current - 1 })
}

// ---- prefs format ----

/// Parse a prefs string like `"text:3"` into `(category, instance)`.
/// Bare names (`"drill"`, `"text"`, etc.) and legacy hyphenated
/// names (`"drill-home"`, `"words-20"`) migrate to the new shape
/// so saved prefs from older versions keep working.
pub fn parse_pref(s: &str) -> (String, usize) {
    // New format: "category:instance"
    if let Some((cat, inst)) = s.split_once(':')
        && CATEGORIES.iter().any(|&c| c == cat)
    {
        let parsed: Option<usize> = match cat {
            "words" => {
                // `words:0` is endless (index 4 in the shipped list).
                // Translate the numeric word-count to an instance
                // index via lookup.
                inst.parse::<u32>()
                    .ok()
                    .and_then(|count| WORDS_INSTANCES.iter().position(|&c| c == count))
            }
            _ => inst.parse().ok(),
        };
        if let Some(i) = parsed {
            return (cat.to_string(), i);
        }
    }

    // Legacy single-name format.
    match s {
        "drill" | "drill-home" | "drill-home-top" | "drill-all" => ("drill".to_string(), 0),
        "words" | "words-20" => ("words".to_string(), 1), // index of 20
        "words-endless" => ("words".to_string(), 4),      // index of 0 (endless)
        "text" => ("text".to_string(), 0),
        _ => ("drill".to_string(), 0),
    }
}

/// Format a `(category, instance)` pair for saving to prefs. Inverse
/// of `parse_pref` for current-format output (legacy strings are
/// never emitted).
pub fn format_pref(category: &str, instance: usize) -> String {
    match category {
        "drill" => "drill".to_string(),
        "words" => {
            let count = WORDS_INSTANCES
                .get(instance)
                .copied()
                .unwrap_or(WORDS_INSTANCES[1]);
            format!("words:{count}")
        }
        "text" => format!("text:{instance}"),
        _ => "drill".to_string(),
    }
}

// ---- drill char set helpers ----

/// Pre-compute the three drill levels' alpha char sets from the
/// active keyboard + layout. Home row is `r = 0`, home+top adds
/// `r = -1`, all-rows covers `r ∈ [-2, 1]`.
fn drill_chars_by_level(keyboard: &dyn Keyboard, layout: &Layout) -> [Vec<char>; 3] {
    [
        alpha_chars_at_rows(keyboard, layout, &[0]),
        alpha_chars_at_rows(keyboard, layout, &[0, -1]),
        alpha_chars_at_rows(keyboard, layout, &[-2, -1, 0, 1]),
    ]
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
