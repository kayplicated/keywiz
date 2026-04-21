//! Simulated annealing layout generator.
//!
//! Starts from a seed layout and iteratively proposes random swaps.
//! Accepts improvements always; accepts regressions probabilistically
//! via the Metropolis criterion. Cooling schedule is linear from
//! `temp_start` to `temp_end` over `iterations` steps.
//!
//! The scoring model is whatever [`crate::score::score`] evaluates,
//! including row-direction, finger-overload, and rolls. This gives
//! drift's generator opinions that diverge from oxey in exactly the
//! axes that matter to the user (flexion, col-stag).

use std::collections::HashMap;

use rand::Rng;
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::config::Config;
use crate::corpus::Corpus;
use crate::keyboard::{Key, Keyboard};
use crate::layout::Layout;
use crate::score;
use crate::trigram::TrigramPipeline;

/// Parameters for a generate run.
#[derive(Debug, Clone)]
pub struct GenerateOptions {
    /// Number of SA iterations.
    pub iterations: usize,
    /// Starting temperature. Higher = more exploratory early on.
    pub temp_start: f64,
    /// Ending temperature. Should approach 0 for a greedy finish.
    pub temp_end: f64,
    /// Letters to pin in their starting positions (never swapped).
    pub pinned: Vec<char>,
    /// RNG seed. Use `None` for system entropy.
    pub seed: Option<u64>,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            iterations: 20_000,
            temp_start: 5.0,
            temp_end: 0.01,
            pinned: Vec::new(),
            seed: None,
        }
    }
}

/// Result of a generate run.
#[derive(Debug, Clone)]
pub struct GenerateResult {
    /// Best layout found, by score.
    pub best: Layout,
    /// Best score.
    pub best_score: f64,
    /// Starting layout's score, for comparison.
    pub initial_score: f64,
    /// Number of iterations run.
    pub iterations: usize,
    /// Number of accepted swaps.
    pub accepted: usize,
    /// Number of "uphill" swaps accepted (Metropolis criterion fired).
    pub uphill_accepted: usize,
}

/// Run simulated annealing on a seed layout.
///
/// Swaps two characters' positions at random each step (subject to
/// pin constraints) and follows the Metropolis-Hastings criterion.
pub fn generate(
    seed: &Layout,
    keyboard: &Keyboard,
    corpus: &Corpus,
    config: &Config,
    pipeline: &TrigramPipeline,
    opts: &GenerateOptions,
) -> GenerateResult {
    let mut rng: StdRng = match opts.seed {
        Some(s) => SeedableRng::seed_from_u64(s),
        None => SeedableRng::from_os_rng(),
    };

    let pinned_set: std::collections::HashSet<char> = opts
        .pinned
        .iter()
        .map(|c| c.to_ascii_lowercase())
        .collect();

    // Build mutable state: vector of (char, Key) pairs. Order doesn't
    // matter; we swap keys between entries during annealing.
    let mut entries: Vec<(char, Key)> = seed
        .positions
        .iter()
        .map(|(&ch, key)| (ch, key.clone()))
        .collect();

    // Precompute which indices are swappable.
    let swappable: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, (ch, _))| (!pinned_set.contains(ch)).then_some(i))
        .collect();

    if swappable.len() < 2 {
        let initial_score = score_from_entries(&entries, seed, keyboard, corpus, config, pipeline);
        return GenerateResult {
            best: seed.clone(),
            best_score: initial_score,
            initial_score,
            iterations: 0,
            accepted: 0,
            uphill_accepted: 0,
        };
    }

    let mut current_score = score_from_entries(&entries, seed, keyboard, corpus, config, pipeline);
    let initial_score = current_score;

    let mut best_entries = entries.clone();
    let mut best_score = current_score;

    let mut accepted = 0usize;
    let mut uphill_accepted = 0usize;

    for step in 0..opts.iterations {
        // Pick two distinct swappable indices.
        let i = swappable[rng.random_range(0..swappable.len())];
        let mut j = swappable[rng.random_range(0..swappable.len())];
        while j == i {
            j = swappable[rng.random_range(0..swappable.len())];
        }

        // Swap the Key geometry between the two chars (not the chars
        // themselves). The char at index i keeps its char but takes
        // the Key of index j, and vice versa.
        let key_i = entries[i].1.clone();
        let key_j = entries[j].1.clone();
        entries[i].1 = key_j;
        entries[j].1 = key_i;

        let new_score = score_from_entries(&entries, seed, keyboard, corpus, config, pipeline);
        let delta = new_score - current_score;

        let temp = cooling_temp(step, opts);
        let accept = if delta >= 0.0 {
            true
        } else {
            let p = (delta / temp).exp();
            rng.random::<f64>() < p
        };

        if accept {
            current_score = new_score;
            accepted += 1;
            if delta < 0.0 {
                uphill_accepted += 1;
            }

            if new_score > best_score {
                best_score = new_score;
                best_entries = entries.clone();
            }
        } else {
            // Revert.
            let key_i = entries[i].1.clone();
            let key_j = entries[j].1.clone();
            entries[i].1 = key_j;
            entries[j].1 = key_i;
        }
    }

    let mut positions = HashMap::new();
    for (ch, key) in best_entries {
        positions.insert(ch, key);
    }

    let best = Layout {
        name: format!("{}-generated", seed.name),
        positions,
    };

    GenerateResult {
        best,
        best_score,
        initial_score,
        iterations: opts.iterations,
        accepted,
        uphill_accepted,
    }
}

/// Linear cooling schedule from `temp_start` to `temp_end`.
fn cooling_temp(step: usize, opts: &GenerateOptions) -> f64 {
    if opts.iterations <= 1 {
        return opts.temp_end;
    }
    let t = step as f64 / (opts.iterations - 1) as f64;
    opts.temp_start + (opts.temp_end - opts.temp_start) * t
}

/// Score a candidate from its entries by temporarily building a
/// [`Layout`] view. Used inside the SA loop.
fn score_from_entries(
    entries: &[(char, Key)],
    seed: &Layout,
    keyboard: &Keyboard,
    corpus: &Corpus,
    config: &Config,
    pipeline: &TrigramPipeline,
) -> f64 {
    let positions: HashMap<char, Key> = entries
        .iter()
        .map(|(ch, key)| (*ch, key.clone()))
        .collect();
    let candidate = Layout {
        name: seed.name.clone(),
        positions,
    };
    score::score(&candidate, keyboard, corpus, config, pipeline).total_score
}
