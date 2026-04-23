//! Pipeline executor.
//!
//! Runs an [`Pipeline`](drift_analyzer::Pipeline) over a corpus and
//! a layout, producing a [`ScoreResult`] with every emitted hit
//! and the total score. The executor is scope-driven: for each
//! scope used by enabled analyzers, it does one pass over the
//! corpus, sharing `WindowProps` across analyzers of that scope.

use std::collections::HashMap;

use drift_analyzer::{AggregateContext, Pipeline};
use drift_core::{CorpusSource, Finger, Hit, Key, Keyboard, Layout, Scope, Window, WindowProps};

/// Result of scoring a layout against a corpus under a pipeline.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ScoreResult {
    /// Total signed score — sum of all hits' costs.
    pub total: f64,
    /// Every emitted hit, in pipeline order.
    pub hits: Vec<Hit>,
    pub layout_name: String,
    pub keyboard_name: String,
    pub corpus_name: String,
}

/// Run `pipeline` over `corpus` on `layout`.
pub fn run(
    pipeline: &Pipeline,
    layout: &Layout,
    keyboard: &Keyboard,
    corpus: &dyn CorpusSource,
) -> ScoreResult {
    let mut hits = Vec::new();

    // Per-char accumulation. Always runs — supplies the
    // AggregateContext later, and powers unigram analyzers.
    let (char_load, finger_load) = accumulate_loads(layout, corpus);
    let has_unigram = pipeline.scopes().contains(&Scope::Unigram);
    if has_unigram {
        run_unigram_pass(pipeline, layout, corpus, &mut hits);
    }

    // Scope passes, in sort order from Pipeline::scopes.
    for scope in pipeline.scopes() {
        match scope {
            Scope::Unigram => {
                // Already handled above.
            }
            Scope::Bigram => run_bigram_pass(pipeline, layout, corpus, &mut hits),
            Scope::Trigram => run_trigram_pass(pipeline, layout, corpus, &mut hits),
            Scope::Ngram(n) => run_ngram_pass(pipeline, layout, corpus, n, &mut hits),
            Scope::Skipgram(gap) => {
                run_skipgram_pass(pipeline, layout, corpus, gap, &mut hits);
            }
            Scope::Aggregate => { /* handled after the match */ }
            _ => {}
        }
    }

    // Aggregate pass last, so it sees the fully-populated
    // per-char / per-finger loads.
    let ctx = AggregateContext {
        layout,
        corpus_name: corpus.name(),
        char_load: &char_load,
        finger_load: &finger_load,
    };
    for analyzer in pipeline.for_scope(Scope::Aggregate) {
        hits.extend(analyzer.evaluate_aggregate(&ctx));
    }

    let total = hits.iter().map(|h| h.cost).sum();
    ScoreResult {
        total,
        hits,
        layout_name: layout.name.clone(),
        keyboard_name: keyboard.name.clone(),
        corpus_name: corpus.name().to_string(),
    }
}

fn accumulate_loads(
    layout: &Layout,
    corpus: &dyn CorpusSource,
) -> (HashMap<char, f64>, HashMap<Finger, f64>) {
    // Sort so summation order is deterministic — floating-point
    // addition isn't associative, and accumulator/delta correctness
    // tests require bit-exact agreement with this path.
    let mut chars_sorted: Vec<(char, f64)> = corpus.iter_chars().collect();
    chars_sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let mut char_load: HashMap<char, f64> = HashMap::new();
    let mut finger_load: HashMap<Finger, f64> = HashMap::new();
    for (ch, freq) in chars_sorted {
        let Some(key) = layout.position(ch) else {
            continue;
        };
        *char_load.entry(ch).or_insert(0.0) += freq;
        *finger_load.entry(key.finger).or_insert(0.0) += freq;
    }
    (char_load, finger_load)
}

fn run_unigram_pass(
    pipeline: &Pipeline,
    layout: &Layout,
    corpus: &dyn CorpusSource,
    hits: &mut Vec<Hit>,
) {
    let mut chars_sorted: Vec<(char, f64)> = corpus.iter_chars().collect();
    chars_sorted.sort_by(|a, b| a.0.cmp(&b.0));
    for (ch, freq) in chars_sorted {
        let Some(key) = layout.position(ch) else {
            continue;
        };
        let chars = [ch];
        let keys = [key];
        let props = build_props(&keys);
        let window = Window {
            chars: &chars,
            keys: &keys,
            freq,
            props: &props,
        };
        for analyzer in pipeline.for_scope(Scope::Unigram) {
            hits.extend(analyzer.evaluate(&window));
        }
    }
}

fn run_bigram_pass(
    pipeline: &Pipeline,
    layout: &Layout,
    corpus: &dyn CorpusSource,
    hits: &mut Vec<Hit>,
) {
    let mut bigrams: Vec<((char, char), f64)> = corpus.iter_bigrams().collect();
    bigrams.sort_by(|a, b| a.0.cmp(&b.0));
    for ((a, b), freq) in bigrams {
        let (Some(ka), Some(kb)) = (layout.position(a), layout.position(b)) else {
            continue;
        };
        let chars = [a, b];
        let keys = [ka, kb];
        let props = build_props(&keys);
        let window = Window {
            chars: &chars,
            keys: &keys,
            freq,
            props: &props,
        };
        for analyzer in pipeline.for_scope(Scope::Bigram) {
            hits.extend(analyzer.evaluate(&window));
        }
    }
}

fn run_trigram_pass(
    pipeline: &Pipeline,
    layout: &Layout,
    corpus: &dyn CorpusSource,
    hits: &mut Vec<Hit>,
) {
    let mut trigrams: Vec<((char, char, char), f64)> = corpus.iter_trigrams().collect();
    trigrams.sort_by(|a, b| a.0.cmp(&b.0));
    for ((a, b, c), freq) in trigrams {
        let (Some(ka), Some(kb), Some(kc)) =
            (layout.position(a), layout.position(b), layout.position(c))
        else {
            continue;
        };
        let chars = [a, b, c];
        let keys = [ka, kb, kc];
        let props = build_props(&keys);
        let window = Window {
            chars: &chars,
            keys: &keys,
            freq,
            props: &props,
        };
        for analyzer in pipeline.for_scope(Scope::Trigram) {
            hits.extend(analyzer.evaluate(&window));
        }
    }
}

fn run_ngram_pass(
    pipeline: &Pipeline,
    layout: &Layout,
    corpus: &dyn CorpusSource,
    n: usize,
    hits: &mut Vec<Hit>,
) {
    let mut ngrams: Vec<(Vec<char>, f64)> = corpus
        .iter_ngrams(n)
        .filter(|(chars, _)| chars.len() == n)
        .collect();
    ngrams.sort_by(|a, b| a.0.cmp(&b.0));
    for (chars_vec, freq) in ngrams {
        let mut keys: Vec<&Key> = Vec::with_capacity(n);
        let mut missed = false;
        for &ch in &chars_vec {
            match layout.position(ch) {
                Some(k) => keys.push(k),
                None => {
                    missed = true;
                    break;
                }
            }
        }
        if missed {
            continue;
        }
        let props = build_props(&keys);
        let window = Window {
            chars: &chars_vec,
            keys: &keys,
            freq,
            props: &props,
        };
        for analyzer in pipeline.for_scope(Scope::Ngram(n)) {
            hits.extend(analyzer.evaluate(&window));
        }
    }
}

fn run_skipgram_pass(
    pipeline: &Pipeline,
    layout: &Layout,
    corpus: &dyn CorpusSource,
    gap: usize,
    hits: &mut Vec<Hit>,
) {
    let mut pairs: Vec<((char, char), f64)> = corpus.iter_skipgrams(gap).collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    for ((a, b), freq) in pairs {
        let (Some(ka), Some(kb)) = (layout.position(a), layout.position(b)) else {
            continue;
        };
        let chars = [a, b];
        let keys = [ka, kb];
        let props = build_props(&keys);
        let window = Window {
            chars: &chars,
            keys: &keys,
            freq,
            props: &props,
        };
        for analyzer in pipeline.for_scope(Scope::Skipgram(gap)) {
            hits.extend(analyzer.evaluate(&window));
        }
    }
}

/// Compute shared `WindowProps` for an already-resolved key slice.
fn build_props(keys: &[&Key]) -> WindowProps {
    let mut same_hand_pairs = Vec::with_capacity(keys.len().saturating_sub(1));
    for pair in keys.windows(2) {
        same_hand_pairs.push(pair[0].finger.same_hand(pair[1].finger));
    }
    let all_same_hand = same_hand_pairs.iter().all(|&x| x);
    let finger_columns: Vec<u8> = keys.iter().map(|k| k.finger.column()).collect();
    let rows: Vec<_> = keys.iter().map(|k| k.row).collect();
    WindowProps {
        same_hand_pairs,
        all_same_hand,
        finger_columns,
        rows,
    }
}
