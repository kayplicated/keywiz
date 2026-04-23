//! Simulated-annealing layout generator.
//!
//! Starts from a seed layout and iteratively proposes random char
//! swaps. Accepts improvements unconditionally, accepts regressions
//! via the Metropolis criterion (probability falls with temperature).
//! Cooling is linear from `temp_start` to `temp_end` across
//! `iterations` steps.
//!
//! The generator is opinion-free: it optimizes for whatever the
//! pipeline's total score is. Plug in a Drifter-preset pipeline
//! and it chases Drifter's scoring philosophy; plug in a neutral
//! or custom pipeline and it chases that.

use std::collections::HashSet;

use anyhow::Result;
use drift_analyzer::Pipeline;
use drift_core::{CorpusSource, Keyboard, Layout};
use drift_delta::ScoreAccumulator;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// Parameters for a generate run.
#[derive(Debug, Clone)]
pub struct SaConfig {
    pub iterations: usize,
    /// Starting temperature. Higher = more exploratory early on.
    pub temp_start: f64,
    /// Ending temperature. Should approach 0 for a greedy finish.
    pub temp_end: f64,
    /// Characters pinned in their starting positions (never swapped).
    pub pinned: Vec<char>,
    /// RNG seed. `None` draws from OS entropy.
    pub seed: Option<u64>,
}

impl Default for SaConfig {
    fn default() -> Self {
        Self {
            iterations: 200_000,
            temp_start: 5.0,
            temp_end: 0.01,
            pinned: Vec::new(),
            seed: None,
        }
    }
}

/// Outcome of a generate run.
#[derive(Debug, Clone)]
pub struct GenerateResult {
    /// Best layout found, by total score.
    pub best: Layout,
    pub best_score: f64,
    pub initial_score: f64,
    pub iterations: usize,
    pub accepted: usize,
    /// Swaps accepted despite a score drop (Metropolis criterion fired).
    pub uphill_accepted: usize,
}

/// Run SA search. If `initial` is `None`, the seed is the corpus-
/// agnostic caller-supplied layout elsewhere; for now we require
/// a seed because `drift-generate` doesn't know where random
/// starting layouts come from.
pub fn generate(
    pipeline: &Pipeline,
    _keyboard: &Keyboard,
    corpus: &dyn CorpusSource,
    seed: Layout,
    config: &SaConfig,
) -> Result<GenerateResult> {
    let mut rng: StdRng = match config.seed {
        Some(s) => SeedableRng::seed_from_u64(s),
        None => SeedableRng::from_os_rng(),
    };

    let pinned: HashSet<char> = config
        .pinned
        .iter()
        .map(|c| c.to_ascii_lowercase())
        .collect();

    let mut layout = seed.clone();
    let mut swappable: Vec<char> = layout
        .positions
        .keys()
        .copied()
        .filter(|c| !pinned.contains(c))
        .collect();
    // HashMap iteration order isn't stable across runs. Sort so the
    // RNG draws from the same index → char mapping on every run with
    // a given seed, making SA results reproducible.
    swappable.sort_unstable();

    let mut accumulator = ScoreAccumulator::init(&layout, corpus, pipeline);
    let initial_score = accumulator.total;
    let mut best_layout = layout.clone();
    let mut best_score = initial_score;
    let mut accepted = 0usize;
    let mut uphill_accepted = 0usize;

    if swappable.len() < 2 {
        return Ok(GenerateResult {
            best: best_layout,
            best_score,
            initial_score,
            iterations: 0,
            accepted: 0,
            uphill_accepted: 0,
        });
    }

    for step in 0..config.iterations {
        let a = swappable[rng.random_range(0..swappable.len())];
        let mut b = swappable[rng.random_range(0..swappable.len())];
        while b == a {
            b = swappable[rng.random_range(0..swappable.len())];
        }

        let candidate = accumulator.swap_delta(&layout, a, b, corpus, pipeline);
        let delta = candidate - accumulator.total;
        let temp = cooling_temp(step, config);

        let accept = if delta >= 0.0 {
            true
        } else {
            rng.random::<f64>() < (delta / temp).exp()
        };

        if accept {
            swap_chars(&mut layout, a, b);
            accumulator.commit_swap(&layout, a, b, corpus, pipeline);
            accepted += 1;
            if delta < 0.0 {
                uphill_accepted += 1;
            }
            if accumulator.total > best_score {
                best_score = accumulator.total;
                best_layout = layout.clone();
            }
        }
    }

    let best = Layout {
        name: format!("{}-generated", seed.name),
        positions: best_layout.positions,
    };
    Ok(GenerateResult {
        best,
        best_score,
        initial_score,
        iterations: config.iterations,
        accepted,
        uphill_accepted,
    })
}

fn swap_chars(layout: &mut Layout, a: char, b: char) {
    if let (Some(ka), Some(kb)) = (layout.positions.remove(&a), layout.positions.remove(&b)) {
        layout.positions.insert(a, kb);
        layout.positions.insert(b, ka);
    }
}

fn cooling_temp(step: usize, config: &SaConfig) -> f64 {
    if config.iterations <= 1 {
        return config.temp_end;
    }
    let t = step as f64 / (config.iterations - 1) as f64;
    config.temp_start + (config.temp_end - config.temp_start) * t
}
