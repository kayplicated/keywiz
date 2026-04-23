//! Heat-weighted word selection.
//!
//! Scores each candidate word by how much it exercises the user's
//! hot keys, then picks one with probability proportional to that
//! score. Each word gets a small baseline weight so cold-letter
//! words still appear — the language stays natural instead of
//! degenerating into "every word must contain X."
//!
//! Returns `None` when no word has any heat to exercise, so
//! callers can fall back to plain uniform random and keep
//! behavior stable for users who haven't accumulated any stats
//! yet.
//!
//! The heat map is the caller's problem — this module takes a
//! `HashMap<char, f32>` of normalized heat values and doesn't
//! know where it came from. Typical caller hands in the output
//! of `keywiz_stats::views::heat::heat_map` filtered by the
//! current layout hash.

use std::collections::HashMap;

use rand::distr::weighted::WeightedIndex;
use rand::distr::Distribution;

/// Baseline score every word gets before heat is added on.
/// Without this, words containing no hot letters would never be
/// picked, flattening the vocabulary. With it, they stay possible
/// but quiet.
const BASELINE_WEIGHT: f64 = 1.0;

/// Pick a word from `words`, weighted by heat overlap. Returns
/// `None` if the heat map is empty (no signal to bias on).
pub fn pick_weighted<'a>(
    words: &[&'a str],
    heat: &HashMap<char, f32>,
) -> Option<&'a str> {
    if words.is_empty() || heat.is_empty() {
        return None;
    }
    let weights: Vec<f64> = words.iter().map(|w| word_weight(w, heat)).collect();
    let dist = WeightedIndex::new(&weights).ok()?;
    let idx = dist.sample(&mut rand::rng());
    Some(words[idx])
}

/// Score for one word: baseline plus heat of each letter in the
/// word. Repeated letters count repeatedly — a word with two hot
/// `t`s exercises `t` twice, so it should weigh twice as much.
fn word_weight(word: &str, heat: &HashMap<char, f32>) -> f64 {
    let heat_sum: f64 = word
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .filter_map(|c| heat.get(&c).copied())
        .map(|h| h as f64)
        .sum();
    BASELINE_WEIGHT + heat_sum
}
