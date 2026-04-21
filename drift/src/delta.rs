//! Delta scoring for the simulated-annealing hot loop.
//!
//! [`ScoreAccumulator`] holds the current aggregate score for a
//! layout plus enough state to recompute only the n-grams whose
//! contribution changes when two characters swap keys. SA
//! iterations call [`ScoreAccumulator::swap_delta`] to preview a
//! candidate swap without rescoring the whole corpus, then
//! [`ScoreAccumulator::commit_swap`] if the swap is accepted.
//!
//! The scoring model is identical to [`crate::score::score`] under
//! [`crate::score::ScoreMode::FastTotalOnly`]: same weights, same
//! low-frequency pruning, same rules. Delta scoring is an
//! optimization, not a different cost function.

use std::collections::HashMap;

use crate::config::Config;
use crate::corpus::Corpus;
use crate::keyboard::Finger;
use crate::layout::Layout;
use crate::motion::{classify, CrossRowKind, Motion, RollDirection};
use crate::trigram::{TrigramContext, TrigramPipeline};

/// Minimum bigram frequency to evaluate (matches score.rs pruning).
const MIN_BIGRAM_FREQ: f64 = 0.001;

/// Minimum trigram frequency to evaluate (matches score.rs pruning).
const MIN_TRIGRAM_FREQ: f64 = 0.01;

/// Running score state that supports O(1)-ish updates per swap.
///
/// Built once at the start of an SA run, then the hot loop only
/// needs to preview and commit swaps. Holds per-char indexes into
/// the corpus so a single swap touches only the ~50 bigrams and
/// ~500 trigrams that contain either character, rather than the
/// entire corpus on every iteration.
pub struct ScoreAccumulator {
    /// Current total score for the layout as it stands.
    pub total: f64,

    /// Per-char load (in percentage points) across the layout.
    /// Kept up to date by [`commit_swap`] so the quadratic finger
    /// overload can be redelta'd cheaply.
    finger_load: HashMap<Finger, f64>,

    /// Current finger-overload contribution to `total`. When a swap
    /// moves chars between fingers, we subtract this and add the
    /// recomputed value.
    finger_overload: f64,

    /// For each char, the bigram pairs in the corpus that contain
    /// it (either as first or second). Used to narrow the set of
    /// ngrams whose contribution changes under a swap.
    bigrams_by_char: HashMap<char, Vec<(char, char, f64)>>,

    /// Same idea for trigrams.
    trigrams_by_char: HashMap<char, Vec<(char, char, char, f64)>>,
}

impl ScoreAccumulator {
    /// Build from a full initial scoring pass over the layout.
    pub fn init(
        layout: &Layout,
        corpus: &Corpus,
        config: &Config,
        pipeline: &TrigramPipeline,
    ) -> Self {
        let mut total = 0.0;
        let mut finger_load: HashMap<Finger, f64> = HashMap::new();
        let mut bigrams_by_char: HashMap<char, Vec<(char, char, f64)>> = HashMap::new();
        let mut trigrams_by_char: HashMap<char, Vec<(char, char, char, f64)>> = HashMap::new();

        // Per-char load.
        for (&ch, &freq) in &corpus.chars {
            let Some(key) = layout.position(ch) else { continue };
            *finger_load.entry(key.finger).or_insert(0.0) += freq;
        }

        // Bigrams.
        for (&(a, b), &freq) in &corpus.bigrams {
            if freq < MIN_BIGRAM_FREQ {
                continue;
            }
            if layout.position(a).is_some() && layout.position(b).is_some() {
                bigrams_by_char.entry(a).or_default().push((a, b, freq));
                if a != b {
                    bigrams_by_char.entry(b).or_default().push((a, b, freq));
                }
                total += bigram_contribution(a, b, freq, layout, config);
            }
        }

        // Trigrams.
        if !pipeline.is_empty() {
            for (&(a, b, c), &freq) in &corpus.trigrams {
                if freq < MIN_TRIGRAM_FREQ {
                    continue;
                }
                if layout.position(a).is_none()
                    || layout.position(b).is_none()
                    || layout.position(c).is_none()
                {
                    continue;
                }
                let key = (a, b, c, freq);
                trigrams_by_char.entry(a).or_default().push(key);
                if b != a {
                    trigrams_by_char.entry(b).or_default().push(key);
                }
                if c != a && c != b {
                    trigrams_by_char.entry(c).or_default().push(key);
                }
                total += trigram_contribution(a, b, c, freq, layout, pipeline);
            }
        }

        // Finger overload.
        let finger_overload = overload_cost(&finger_load, config);
        total += finger_overload;

        ScoreAccumulator {
            total,
            finger_load,
            finger_overload,
            bigrams_by_char,
            trigrams_by_char,
        }
    }

    /// Return the score the layout WOULD have if chars `a` and `b`
    /// swapped keys right now. Does not mutate any state.
    ///
    /// The caller has to guarantee that `a != b` and that both
    /// chars are present in the layout.
    pub fn swap_delta(
        &self,
        layout: &Layout,
        a: char,
        b: char,
        corpus: &Corpus,
        config: &Config,
        pipeline: &TrigramPipeline,
    ) -> f64 {
        // Sum of all affected bigram/trigram contributions at the
        // current (pre-swap) positions. This is what we subtract.
        let before = self.contribution_of_chars(layout, a, b, config, pipeline);

        // Sum of all affected contributions if we temporarily
        // imagine a and b have swapped keys.
        let swapped = swapped_layout(layout, a, b);
        let after = self.contribution_of_chars(&swapped, a, b, config, pipeline);

        // Finger overload: only recompute if the swap moves chars
        // between fingers. Otherwise finger_load is unchanged and
        // the existing contribution stands.
        let (ka, kb) = match (layout.position(a), layout.position(b)) {
            (Some(ka), Some(kb)) => (ka, kb),
            _ => return self.total - before + after,
        };
        let overload_after = if ka.finger == kb.finger {
            self.finger_overload
        } else {
            let freq_a = corpus_char_freq(corpus, a);
            let freq_b = corpus_char_freq(corpus, b);
            let mut tmp = self.finger_load.clone();
            // Remove each char from its current finger, add it to
            // the other's finger. ka is a's current finger, kb is
            // b's current finger — after the swap they'd reverse.
            *tmp.entry(ka.finger).or_insert(0.0) -= freq_a;
            *tmp.entry(kb.finger).or_insert(0.0) -= freq_b;
            *tmp.entry(ka.finger).or_insert(0.0) += freq_b;
            *tmp.entry(kb.finger).or_insert(0.0) += freq_a;
            overload_cost(&tmp, config)
        };

        self.total - before + after - self.finger_overload + overload_after
    }

    /// Apply a swap to the accumulator. Caller is expected to have
    /// performed the actual layout mutation already.
    pub fn commit_swap(
        &mut self,
        layout_after_swap: &Layout,
        a: char,
        b: char,
        corpus: &Corpus,
        config: &Config,
        pipeline: &TrigramPipeline,
    ) {
        // Rebuild the affected contributions. We need the old values
        // relative to the pre-swap layout, which we reconstruct by
        // calling with a temporarily-swapped copy.
        let pre_swap = swapped_layout(layout_after_swap, a, b);
        let before = self.contribution_of_chars(&pre_swap, a, b, config, pipeline);
        let after = self.contribution_of_chars(layout_after_swap, a, b, config, pipeline);

        self.total = self.total - before + after;

        // Finger overload: update finger_load then recompute.
        if let (Some(ka), Some(kb)) = (layout_after_swap.position(a), layout_after_swap.position(b))
        {
            if ka.finger != kb.finger {
                let freq_a = corpus_char_freq(corpus, a);
                let freq_b = corpus_char_freq(corpus, b);
                // After the swap: char a now lives on ka.finger with freq_a,
                // char b now on kb.finger with freq_b. Before the swap a was
                // on kb's finger, b on ka's finger.
                *self.finger_load.entry(kb.finger).or_insert(0.0) -= freq_a;
                *self.finger_load.entry(ka.finger).or_insert(0.0) -= freq_b;
                *self.finger_load.entry(ka.finger).or_insert(0.0) += freq_a;
                *self.finger_load.entry(kb.finger).or_insert(0.0) += freq_b;
            }
        }
        let new_overload = overload_cost(&self.finger_load, config);
        self.total = self.total - self.finger_overload + new_overload;
        self.finger_overload = new_overload;
    }

    /// Sum the bigram and trigram contributions that touch either
    /// `a` or `b`, evaluated against the positions in `layout`.
    ///
    /// Deduplicates cases where an ngram contains both chars.
    fn contribution_of_chars(
        &self,
        layout: &Layout,
        a: char,
        b: char,
        config: &Config,
        pipeline: &TrigramPipeline,
    ) -> f64 {
        let mut sum = 0.0;

        // Bigrams: sum from a's list, then add b's list but skip
        // pairs that already contain a (avoid double-counting).
        if let Some(list) = self.bigrams_by_char.get(&a) {
            for &(x, y, freq) in list {
                sum += bigram_contribution(x, y, freq, layout, config);
            }
        }
        if let Some(list) = self.bigrams_by_char.get(&b) {
            for &(x, y, freq) in list {
                if x == a || y == a {
                    continue; // already summed via a's list
                }
                sum += bigram_contribution(x, y, freq, layout, config);
            }
        }

        // Trigrams: same logic.
        if !pipeline.is_empty() {
            if let Some(list) = self.trigrams_by_char.get(&a) {
                for &(x, y, z, freq) in list {
                    sum += trigram_contribution(x, y, z, freq, layout, pipeline);
                }
            }
            if let Some(list) = self.trigrams_by_char.get(&b) {
                for &(x, y, z, freq) in list {
                    if x == a || y == a || z == a {
                        continue;
                    }
                    sum += trigram_contribution(x, y, z, freq, layout, pipeline);
                }
            }
        }

        sum
    }

}

/// Look up a char's frequency from the corpus (%).
fn corpus_char_freq(corpus: &Corpus, ch: char) -> f64 {
    corpus.chars.get(&ch).copied().unwrap_or(0.0)
}

/// Swap the positions of chars `a` and `b` in a copy of `layout`.
fn swapped_layout(layout: &Layout, a: char, b: char) -> Layout {
    let mut positions = layout.positions.clone();
    if let (Some(ka), Some(kb)) = (positions.remove(&a), positions.remove(&b)) {
        positions.insert(a, kb);
        positions.insert(b, ka);
    }
    Layout {
        name: layout.name.clone(),
        positions,
    }
}

/// Score a single bigram's contribution (frequency-weighted motion
/// cost or roll bonus), using the same logic as score.rs.
fn bigram_contribution(a: char, b: char, freq: f64, layout: &Layout, config: &Config) -> f64 {
    let (Some(ka), Some(kb)) = (layout.position(a), layout.position(b)) else {
        return 0.0;
    };
    let motion = classify(ka, kb, &config.asymmetric);
    match motion {
        Motion::Alternate | Motion::SameKey | Motion::SameRowSkip { .. }
        | Motion::AdjacentForwardOk { .. } => 0.0,
        Motion::Sfb { .. } => freq * config.bigram.sfb_penalty,
        Motion::Roll { direction, .. } => {
            let mult = match direction {
                RollDirection::Inward => config.roll.inward_multiplier,
                RollDirection::Outward => config.roll.outward_multiplier,
            };
            freq * config.roll.same_row_adjacent * mult
        }
        Motion::CrossRow { kind, .. } => {
            let mult = match kind {
                CrossRowKind::Flexion => config.row.flexion,
                CrossRowKind::Extension => config.row.extension,
                CrossRowKind::FullCross => config.row.full_cross,
            };
            freq * config.bigram.scissor_penalty * mult
        }
        Motion::Stretch { .. } => freq * config.bigram.lateral_penalty,
    }
}

/// Score a single trigram against every enabled rule. Same as
/// score.rs's trigram loop but extracted for reuse.
fn trigram_contribution(
    a: char,
    b: char,
    c: char,
    freq: f64,
    layout: &Layout,
    pipeline: &TrigramPipeline,
) -> f64 {
    let (Some(ka), Some(kb), Some(kc)) = (
        layout.position(a),
        layout.position(b),
        layout.position(c),
    ) else {
        return 0.0;
    };
    let ctx = TrigramContext::new([a, b, c], [ka, kb, kc], freq);
    let mut sum = 0.0;
    for rule in &pipeline.rules {
        if let Some(hit) = rule.evaluate(&ctx) {
            sum += hit.cost;
        }
    }
    sum
}

/// Quadratic finger-overload cost. Identical formula to score.rs.
fn overload_cost(finger_load: &HashMap<Finger, f64>, config: &Config) -> f64 {
    let fingers = [
        Finger::LPinky, Finger::LRing, Finger::LMiddle, Finger::LIndex,
        Finger::RIndex, Finger::RMiddle, Finger::RRing, Finger::RPinky,
    ];
    let total_weight: f64 = fingers.iter().map(|&f| finger_weight(f, config)).sum();
    let total_load: f64 = finger_load.values().sum();
    fingers
        .iter()
        .map(|&f| {
            let load = finger_load.get(&f).copied().unwrap_or(0.0);
            let fair_share = total_load * finger_weight(f, config) / total_weight;
            let excess = (load - fair_share).max(0.0);
            excess * excess
        })
        .sum::<f64>()
        * config.finger.overload_penalty
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
