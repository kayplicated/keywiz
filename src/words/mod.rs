//! Word selection for typing modes.
//!
//! The public API is [`random_word`], which returns the next word to type.
//! Behavior silently adapts to the user's per-key heat: words containing
//! hot letters become more likely to appear, so struggling keys get more
//! practice without any explicit "drill X" UI. With no heat anywhere, the
//! selection is uniform random over the word list.

pub mod heated;

use rand::prelude::IndexedRandom;

use crate::stats::Stats;

const WORDS: &str = include_str!("words.txt");

fn all_words() -> Vec<&'static str> {
    WORDS.lines().filter(|l| !l.is_empty()).collect()
}

/// Pick the next word to type. Uses heat-weighted selection when any key
/// has heat, falls back to uniform random otherwise.
pub fn random_word(stats: &Stats) -> String {
    let words = all_words();
    heated::pick_weighted(&words, stats)
        .or_else(|| words.choose(&mut rand::rng()).copied())
        .unwrap_or("the")
        .to_string()
}
