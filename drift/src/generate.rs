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

use rand::Rng;
use rand::rngs::StdRng;
use rand::SeedableRng;

use crate::config::Config;
use crate::corpus::Corpus;
use crate::delta::ScoreAccumulator;
use crate::keyboard::Keyboard;
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
    _keyboard: &Keyboard,
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

    // Running layout state: start from the seed, mutate in place.
    let mut layout = Layout {
        name: seed.name.clone(),
        positions: seed.positions.clone(),
    };

    // Only these chars may be swapped.
    let swappable: Vec<char> = layout
        .positions
        .keys()
        .copied()
        .filter(|ch| !pinned_set.contains(ch))
        .collect();

    if swappable.len() < 2 {
        let accumulator = ScoreAccumulator::init(&layout, corpus, config, pipeline);
        let initial_score = accumulator.total;
        return GenerateResult {
            best: layout.clone(),
            best_score: initial_score,
            initial_score,
            iterations: 0,
            accepted: 0,
            uphill_accepted: 0,
        };
    }

    // Accumulator holds the live total + enough indexes to score a
    // swap in O(affected ngrams) instead of full O(corpus).
    let mut accumulator = ScoreAccumulator::init(&layout, corpus, config, pipeline);
    let initial_score = accumulator.total;
    let mut best_layout = layout.clone();
    let mut best_score = accumulator.total;

    let mut accepted = 0usize;
    let mut uphill_accepted = 0usize;

    for step in 0..opts.iterations {
        // Pick two distinct swappable chars.
        let a = swappable[rng.random_range(0..swappable.len())];
        let mut b = swappable[rng.random_range(0..swappable.len())];
        while b == a {
            b = swappable[rng.random_range(0..swappable.len())];
        }

        // Preview the swap's impact on the total score without
        // touching any state.
        let candidate_score =
            accumulator.swap_delta(&layout, a, b, corpus, config, pipeline);
        let delta = candidate_score - accumulator.total;

        let temp = cooling_temp(step, opts);
        let accept = if delta >= 0.0 {
            true
        } else {
            let p = (delta / temp).exp();
            rng.random::<f64>() < p
        };

        if accept {
            swap_chars_in_layout(&mut layout, a, b);
            accumulator.commit_swap(&layout, a, b, corpus, config, pipeline);
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

    // Consistency check: the delta-tracked `best_score` should
    // agree with a fresh FastTotalOnly rescore to ~epsilon. If
    // they diverge more than 0.01 we've accumulated drift — which
    // would be a bug in ScoreAccumulator.
    if std::env::var("DRIFT_CHECK_DELTA").is_ok() {
        let fresh = score::score(
            &best,
            _keyboard,
            corpus,
            config,
            pipeline,
            score::ScoreMode::FastTotalOnly,
        )
        .total_score;
        let gap = (best_score - fresh).abs();
        if gap > 0.01 {
            eprintln!(
                "delta-score drift: best_score={:.6} fresh={:.6} gap={:.6}",
                best_score, fresh, gap
            );
        } else {
            eprintln!("delta-score OK: gap={:.6}", gap);
        }
    }

    GenerateResult {
        best,
        best_score,
        initial_score,
        iterations: opts.iterations,
        accepted,
        uphill_accepted,
    }
}

/// Swap the positions of chars `a` and `b` in the layout in place.
fn swap_chars_in_layout(layout: &mut Layout, a: char, b: char) {
    if let (Some(ka), Some(kb)) = (
        layout.positions.remove(&a),
        layout.positions.remove(&b),
    ) {
        layout.positions.insert(a, kb);
        layout.positions.insert(b, ka);
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

