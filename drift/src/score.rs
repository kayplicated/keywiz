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
pub fn score(
    layout: &Layout,
    keyboard: &Keyboard,
    corpus: &Corpus,
    config: &Config,
) -> ScoreReport {
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

    // Per-bigram scoring.
    for (&(a, b), &freq) in &corpus.bigrams {
        let (Some(ka), Some(kb)) = (layout.position(a), layout.position(b)) else {
            continue;
        };

        let motion = classify(ka, kb, &config.asymmetric);
        apply_motion(motion, freq, ka, kb, a, b, config, &mut tally,
                     &mut sfb_details, &mut scissor_details, &mut roll_details);
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
        + finger_overload_cost;

    // Sort detail vectors and keep top 10 of each.
    sort_and_truncate(&mut sfb_details, 10);
    sort_and_truncate(&mut scissor_details, 10);
    sort_and_truncate(&mut roll_details, 10);

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
            sfb_details.push(BigramDetail {
                pair: (a, b),
                freq,
                contribution: cost,
                label: format!("SFB {}{}", a, b),
            });
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
            scissor_details.push(BigramDetail {
                pair: (a, b),
                freq,
                contribution: cost,
                label: format!("{:?} {}{}", kind, a, b),
            });
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
