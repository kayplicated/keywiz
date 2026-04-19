//! Heat-weighted word selection.
//!
//! Scores each candidate word by how much it exercises the user's hot keys,
//! then picks one with probability proportional to that score. Each word
//! gets a small baseline weight so cold-letter words still appear — the
//! language stays natural instead of degenerating into "every word must
//! contain X."
//!
//! Returns `None` when no word has any heat to exercise, so callers can
//! fall back to plain uniform random and keep behavior stable for users
//! who haven't accumulated any stats yet.

use rand::distr::weighted::WeightedIndex;
use rand::distr::Distribution;

use crate::stats::Stats;

/// Baseline score every word gets before heat is added on. Without this,
/// words containing no hot letters would never be picked, flattening the
/// vocabulary. With it, they stay possible but quiet.
const BASELINE_WEIGHT: f64 = 1.0;

/// Pick a word from `words`, weighted by heat overlap. Returns `None` if
/// the sum of heat across all keys is zero (no signal to bias on).
pub fn pick_weighted<'a>(words: &[&'a str], stats: &Stats) -> Option<&'a str> {
    if words.is_empty() || total_heat(stats) == 0 {
        return None;
    }
    let weights: Vec<f64> = words.iter().map(|w| word_weight(w, stats)).collect();
    let dist = WeightedIndex::new(&weights).ok()?;
    let idx = dist.sample(&mut rand::rng());
    Some(words[idx])
}

/// Sum of all per-key heat. Zero means no signal → skip weighting.
fn total_heat(stats: &Stats) -> u64 {
    stats
        .iter()
        .map(|(_, r)| r.heat as u64)
        .sum()
}

/// Score for one word: baseline plus heat of each letter in the word.
/// Repeated letters count repeatedly — a word with two hot `t`s exercises
/// `t` twice, so it should weigh twice as much.
fn word_weight(word: &str, stats: &Stats) -> f64 {
    let heat_sum: u32 = word
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .filter_map(|c| stats.get(c))
        .map(|r| r.heat)
        .sum();
    BASELINE_WEIGHT + heat_sum as f64
}

