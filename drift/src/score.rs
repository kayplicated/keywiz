//! Scoring pipeline.
//!
//! For every bigram in the corpus, classify its motion on the
//! layout and accumulate weighted cost/reward. Produces a
//! [`ScoreReport`] with aggregate score + breakdown.

use std::collections::HashMap;

use serde::Serialize;

use crate::config::Config;
use crate::corpus::Corpus;
use crate::keyboard::{Finger, Key, Keyboard};
use crate::layout::Layout;
use crate::motion::{classify, CrossRowKind, Motion, RollDirection};
use crate::trigram::{TrigramContext, TrigramPipeline};

/// Minimum bigram frequency (%) to apply full motion classification.
/// Extremely rare bigrams contribute noise to the score and aren't
/// worth the per-iteration cost in SA. Tune downward to capture
/// more; tune upward for faster generation.
const MIN_BIGRAM_FREQ: f64 = 0.001;

/// Minimum trigram frequency (%) to evaluate against rules. Same
/// rationale as [`MIN_BIGRAM_FREQ`]. English trigrams have a long
/// tail below 0.01% that collectively barely moves the score.
const MIN_TRIGRAM_FREQ: f64 = 0.01;

/// What to compute when scoring a layout.
///
/// `Full` is used by the CLI report path and captures every per-
/// bigram and per-trigram detail for display. `FastTotalOnly` is
/// used by simulated annealing: skip detail collection, skip very
/// low-frequency n-grams, return only aggregate totals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreMode {
    Full,
    FastTotalOnly,
}

/// Detailed score breakdown for a single layout.
#[derive(Debug, Clone, Serialize)]
pub struct ScoreReport {
    pub layout_name: String,
    pub keyboard_name: String,
    pub corpus_name: String,

    /// Overall weighted score. Higher = better.
    pub total_score: f64,

    /// Quadratic finger-overload penalty, already folded into
    /// `total_score`. Exposed separately for reports.
    pub finger_overload_cost: f64,

    /// Row distribution (% of typing by row).
    pub row_pct: RowDistribution,

    /// Per-finger load (% typed on each finger).
    pub finger_pct: HashMap<Finger, f64>,

    /// Per-finger strength-weighted load score (`load_pct / weight`).
    pub finger_load: HashMap<Finger, f64>,

    /// Bigram classifications — aggregated frequencies and costs.
    pub motions: MotionTally,

    /// Top-N specific bigrams by contribution (for display).
    pub top_sfbs: Vec<BigramDetail>,
    pub top_scissors: Vec<BigramDetail>,
    pub top_rolls: Vec<BigramDetail>,

    /// Per-category trigram contributions, one entry per rule that fired.
    pub trigram_categories: Vec<TrigramCategory>,

    /// Top trigram hits per category (bounded list for reporting).
    pub top_trigrams: Vec<TrigramDetail>,

    /// Sum of all trigram rule contributions. Already folded into
    /// `total_score`; exposed separately for reporting.
    pub trigram_cost: f64,
}

/// Aggregate of all trigram hits under a single rule category.
#[derive(Debug, Clone, Serialize)]
pub struct TrigramCategory {
    pub name: String,
    pub trigram_pct: f64,
    pub total_cost: f64,
}

/// A single trigram's contribution, for detailed reports.
#[derive(Debug, Clone, Serialize)]
pub struct TrigramDetail {
    pub category: String,
    pub label: String,
    pub freq: f64,
    pub contribution: f64,
}

/// Per-row percentages.
#[derive(Debug, Clone, Default, Serialize)]
pub struct RowDistribution {
    pub top: f64,
    pub home: f64,
    pub bot: f64,
}

/// Aggregate tally across all scored bigrams.
#[derive(Debug, Clone, Default, Serialize)]
pub struct MotionTally {
    pub alternate_pct: f64,
    pub same_key_pct: f64,
    pub sfb_pct: f64,
    pub roll_inward_pct: f64,
    pub roll_outward_pct: f64,
    pub same_row_skip_pct: f64,
    pub cross_row_flexion_pct: f64,
    pub cross_row_extension_pct: f64,
    pub cross_row_full_pct: f64,
    pub cross_row_exempt_pct: f64,
    pub stretch_pct: f64,

    pub sfb_cost: f64,
    pub scissor_cost: f64,
    pub stretch_cost: f64,
    pub roll_bonus: f64,
}

/// A single bigram's classification + cost, for reporting.
#[derive(Debug, Clone, Serialize)]
pub struct BigramDetail {
    /// Source char pair; retained for JSON export and debugging.
    #[allow(dead_code)]
    pub pair: (char, char),
    pub freq: f64,
    pub contribution: f64,
    pub label: String,
}

/// Run the scoring pipeline.
///
/// The caller must pre-build the trigram pipeline (or pass an empty
/// one via [`TrigramPipeline::empty`]) so the hot path stays free of
/// config-parsing errors. In practice [`crate::cli`] builds the
/// pipeline once at startup and reuses it across corpora.
///
/// `mode` controls what information is accumulated. The CLI uses
/// [`ScoreMode::Full`]; SA uses [`ScoreMode::FastTotalOnly`] to skip
/// detail collection and prune low-frequency n-grams.
pub fn score(
    layout: &Layout,
    keyboard: &Keyboard,
    corpus: &Corpus,
    config: &Config,
    pipeline: &TrigramPipeline,
    mode: ScoreMode,
) -> ScoreReport {
    let collect_details = mode == ScoreMode::Full;
    let prune = mode == ScoreMode::FastTotalOnly;
    let mut tally = MotionTally::default();
    let mut row_pct = RowDistribution::default();
    let mut finger_pct: HashMap<Finger, f64> = HashMap::new();

    let mut sfb_details: Vec<BigramDetail> = Vec::new();
    let mut scissor_details: Vec<BigramDetail> = Vec::new();
    let mut roll_details: Vec<BigramDetail> = Vec::new();

    // Per-char row and finger accumulation.
    for (&ch, freq) in &corpus.chars {
        let Some(key) = layout.position(ch) else { continue };
        match key.row {
            -1 => row_pct.top += freq,
            0 => row_pct.home += freq,
            1 => row_pct.bot += freq,
            _ => {}
        }
        *finger_pct.entry(key.finger).or_insert(0.0) += freq;
    }

    // Per-bigram scoring. In fast mode, prune bigrams below the
    // frequency threshold — they contribute noise at SA timescales.
    for (&(a, b), &freq) in &corpus.bigrams {
        if prune && freq < MIN_BIGRAM_FREQ {
            continue;
        }
        let (Some(ka), Some(kb)) = (layout.position(a), layout.position(b)) else {
            continue;
        };

        let motion = classify(ka, kb, &config.asymmetric);
        apply_motion(
            motion, freq, ka, kb, a, b, config, collect_details,
            &mut tally,
            &mut sfb_details,
            &mut scissor_details,
            &mut roll_details,
        );
    }

    // Per-trigram scoring. Each active rule gets a chance at every
    // trigram; hits accumulate by category.
    let mut trigram_cost = 0.0;
    let mut category_totals: HashMap<&'static str, (f64, f64)> = HashMap::new();
    let mut trigram_details: Vec<TrigramDetail> = Vec::new();

    if !pipeline.is_empty() {
        for (&(a, b, c), &freq) in &corpus.trigrams {
            if prune && freq < MIN_TRIGRAM_FREQ {
                continue;
            }
            let (Some(ka), Some(kb), Some(kc)) = (
                layout.position(a),
                layout.position(b),
                layout.position(c),
            ) else {
                continue;
            };
            let ctx = TrigramContext::new([a, b, c], [ka, kb, kc], freq);
            for rule in &pipeline.rules {
                if let Some(hit) = rule.evaluate(&ctx) {
                    trigram_cost += hit.cost;
                    if collect_details {
                        let entry = category_totals.entry(hit.category).or_insert((0.0, 0.0));
                        entry.0 += freq;
                        entry.1 += hit.cost;
                        trigram_details.push(TrigramDetail {
                            category: hit.category.to_string(),
                            label: hit.label,
                            freq,
                            contribution: hit.cost,
                        });
                    }
                }
            }
        }
    }

    // Strength-weighted finger load: load / weight.
    let finger_load: HashMap<Finger, f64> = finger_pct
        .iter()
        .map(|(&f, &load)| (f, load / finger_weight(f, config)))
        .collect();

    // Finger overload: each finger has a "fair share" of load based
    // on its relative strength weight. Load BEYOND that fair share
    // is penalized quadratically, so a wildly-overloaded pinky hurts
    // much more than a balanced distribution.
    //
    // Total typeable load is ~100% (sum of char frequencies). Split
    // that proportionally by finger weights, then penalize excess.
    let total_weight: f64 = [
        Finger::LPinky, Finger::LRing, Finger::LMiddle, Finger::LIndex,
        Finger::RIndex, Finger::RMiddle, Finger::RRing, Finger::RPinky,
    ]
    .iter()
    .map(|&f| finger_weight(f, config))
    .sum();
    let total_load: f64 = finger_pct.values().sum();
    let finger_overload_cost: f64 = finger_pct
        .iter()
        .map(|(&f, &load)| {
            let fair_share = total_load * finger_weight(f, config) / total_weight;
            let excess = (load - fair_share).max(0.0);
            excess * excess
        })
        .sum::<f64>()
        * config.finger.overload_penalty;

    // Aggregate score.
    let total_score = tally.roll_bonus
        + tally.sfb_cost
        + tally.scissor_cost
        + tally.stretch_cost
        + finger_overload_cost
        + trigram_cost;

    // Sort detail vectors and keep top 10 of each.
    sort_and_truncate(&mut sfb_details, 10);
    sort_and_truncate(&mut scissor_details, 10);
    sort_and_truncate(&mut roll_details, 10);

    // Roll up trigram categories.
    let mut trigram_categories: Vec<TrigramCategory> = category_totals
        .into_iter()
        .map(|(name, (pct, cost))| TrigramCategory {
            name: name.to_string(),
            trigram_pct: pct,
            total_cost: cost,
        })
        .collect();
    trigram_categories.sort_by(|a, b| {
        b.total_cost
            .abs()
            .partial_cmp(&a.total_cost.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Sort + trim trigram details. Keep top 10 by absolute
    // contribution across all categories, for the headline list.
    trigram_details.sort_by(|a, b| {
        b.contribution
            .abs()
            .partial_cmp(&a.contribution.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    trigram_details.truncate(30);

    ScoreReport {
        layout_name: layout.name.clone(),
        keyboard_name: keyboard.name.clone(),
        corpus_name: corpus.name.clone(),
        total_score,
        finger_overload_cost,
        row_pct,
        finger_pct,
        finger_load,
        motions: tally,
        top_sfbs: sfb_details,
        top_scissors: scissor_details,
        top_rolls: roll_details,
        trigram_categories,
        top_trigrams: trigram_details,
        trigram_cost,
    }
}

fn apply_motion(
    motion: Motion,
    freq: f64,
    ka: &Key,
    kb: &Key,
    a: char,
    b: char,
    config: &Config,
    collect_details: bool,
    tally: &mut MotionTally,
    sfb_details: &mut Vec<BigramDetail>,
    scissor_details: &mut Vec<BigramDetail>,
    roll_details: &mut Vec<BigramDetail>,
) {
    let _ = (ka, kb); // geometry already consumed by classifier

    match motion {
        Motion::Alternate => {
            tally.alternate_pct += freq;
        }
        Motion::SameKey => {
            tally.same_key_pct += freq;
        }
        Motion::Sfb { .. } => {
            tally.sfb_pct += freq;
            let cost = freq * config.bigram.sfb_penalty;
            tally.sfb_cost += cost;
            if collect_details {
                sfb_details.push(BigramDetail {
                    pair: (a, b),
                    freq,
                    contribution: cost,
                    label: format!("SFB {}{}", a, b),
                });
            }
        }
        Motion::Roll { direction, .. } => {
            let mult = match direction {
                RollDirection::Inward => config.roll.inward_multiplier,
                RollDirection::Outward => config.roll.outward_multiplier,
            };
            let bonus = freq * config.roll.same_row_adjacent * mult;
            tally.roll_bonus += bonus;
            match direction {
                RollDirection::Inward => tally.roll_inward_pct += freq,
                RollDirection::Outward => tally.roll_outward_pct += freq,
            }
            if collect_details {
                roll_details.push(BigramDetail {
                    pair: (a, b),
                    freq,
                    contribution: bonus,
                    label: format!(
                        "{} {}{}",
                        match direction {
                            RollDirection::Inward => "Inward",
                            RollDirection::Outward => "Outward",
                        },
                        a,
                        b
                    ),
                });
            }
        }
        Motion::SameRowSkip { .. } => {
            tally.same_row_skip_pct += freq;
        }
        Motion::CrossRow { kind, .. } => {
            let mult = match kind {
                CrossRowKind::Flexion => config.row.flexion,
                CrossRowKind::Extension => config.row.extension,
                CrossRowKind::FullCross => config.row.full_cross,
            };
            let cost = freq * config.bigram.scissor_penalty * mult;
            tally.scissor_cost += cost;
            match kind {
                CrossRowKind::Flexion => tally.cross_row_flexion_pct += freq,
                CrossRowKind::Extension => tally.cross_row_extension_pct += freq,
                CrossRowKind::FullCross => tally.cross_row_full_pct += freq,
            }
            if collect_details {
                scissor_details.push(BigramDetail {
                    pair: (a, b),
                    freq,
                    contribution: cost,
                    label: format!("{:?} {}{}", kind, a, b),
                });
            }
        }
        Motion::AdjacentForwardOk { .. } => {
            tally.cross_row_exempt_pct += freq;
        }
        Motion::Stretch { .. } => {
            tally.stretch_pct += freq;
            let cost = freq * config.bigram.lateral_penalty;
            tally.stretch_cost += cost;
        }
    }
}

fn finger_weight(f: Finger, config: &Config) -> f64 {
    use Finger::*;
    let fw = &config.finger;
    match f {
        LPinky => fw.left_pinky,
        LRing => fw.left_ring,
        LMiddle => fw.left_middle,
        LIndex => fw.left_index,
        RIndex => fw.right_index,
        RMiddle => fw.right_middle,
        RRing => fw.right_ring,
        RPinky => fw.right_pinky,
    }
}

fn sort_and_truncate(details: &mut Vec<BigramDetail>, n: usize) {
    details.sort_by(|a, b| a.contribution.abs().partial_cmp(&b.contribution.abs()).unwrap_or(std::cmp::Ordering::Equal).reverse());
    details.truncate(n);
}
