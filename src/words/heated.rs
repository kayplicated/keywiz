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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_heat_returns_none() {
        let s = Stats::new();
        assert!(pick_weighted(&["the", "a", "cat"], &s).is_none());
    }

    #[test]
    fn empty_word_list_returns_none() {
        let mut s = Stats::new();
        s.record('a', false);
        assert!(pick_weighted(&[], &s).is_none());
    }

    #[test]
    fn word_weight_counts_repeats() {
        let mut s = Stats::new();
        // Heat 3 on 't'
        for _ in 0..3 {
            s.record('t', false);
        }
        // "the" has one 't' → 1*3 + baseline = 4
        // "tot" has two 't's → 2*3 + baseline = 7
        let w_the = word_weight("the", &s);
        let w_tot = word_weight("tot", &s);
        assert!(w_tot > w_the, "expected tot ({w_tot}) > the ({w_the})");
    }

    #[test]
    fn cold_letter_words_still_have_baseline_weight() {
        let mut s = Stats::new();
        s.record('x', false);
        // "apple" contains no hot letters, still weighs BASELINE_WEIGHT.
        assert_eq!(word_weight("apple", &s), BASELINE_WEIGHT);
    }

    #[test]
    fn picked_word_skews_toward_heat() {
        // With strong heat on 'z', a pool of one z-word plus many cold
        // words should pick the z-word most of the time.
        let mut s = Stats::new();
        for _ in 0..20 {
            s.record('z', false);
        }
        let words = ["apple", "banana", "cherry", "dog", "egg", "zzz"];
        let mut hits_zzz = 0;
        for _ in 0..1000 {
            if pick_weighted(&words, &s) == Some("zzz") {
                hits_zzz += 1;
            }
        }
        // "zzz" weighs 1 + 3*20 = 61. Others weigh 1 each. Total = 66.
        // Expected hit rate ≈ 61/66 ≈ 92%.
        assert!(hits_zzz > 800, "expected z-word to dominate, got {hits_zzz}/1000");
    }
}
