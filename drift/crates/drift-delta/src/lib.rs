//! Incremental scoring for SA hot loops.
//!
//! `ScoreAccumulator` caches a per-char index over every window
//! that touches each character. A candidate swap of `a↔b` rescores
//! only the windows whose char set intersects `{a, b}`, plus the
//! aggregate analyzers. Windows that don't contain either char
//! keep their cached contribution unchanged.
//!
//! The aggregate pass runs every swap, but that's cheap: it
//! evaluates against pre-summarized `char_load` and `finger_load`
//! hashmaps, not the raw corpus.

use std::collections::{HashMap, HashSet};

use drift_analyzer::{AggregateContext, Pipeline};
use drift_core::{CorpusSource, Finger, Key, Layout, Scope, Window, WindowProps};

/// Precomputed per-window frequency data for incremental scoring.
/// One entry per n-gram length actually used by the pipeline's
/// enabled analyzers.
struct WindowIndex {
    /// Every window in the corpus of this length, with its frequency.
    windows: Vec<(Vec<char>, f64)>,
    /// `char → [indices into `windows`]` — any window containing
    /// that character.
    by_char: HashMap<char, Vec<usize>>,
}

/// Running score state that supports O(affected-windows) updates
/// per simulated-annealing swap.
pub struct ScoreAccumulator {
    pub total: f64,

    /// Bigram-scope index. `None` if no bigram analyzers are enabled.
    bigram_index: Option<WindowIndex>,
    /// Trigram-scope index.
    trigram_index: Option<WindowIndex>,
    /// N-gram indexes keyed by length.
    ngram_indexes: HashMap<usize, WindowIndex>,
    /// Skipgram indexes keyed by gap.
    skipgram_indexes: HashMap<usize, WindowIndex>,

    /// Per-char load (%). Kept up to date by `commit_swap`.
    char_load: HashMap<char, f64>,
    /// Per-finger load (%). Derived from `char_load` and the layout.
    finger_load: HashMap<Finger, f64>,
    /// Last known aggregate-hit contribution.
    aggregate_cost: f64,

    /// Cached non-affected contribution — the sum of every window's
    /// cost from the previous full pass, minus any windows currently
    /// being evaluated during a swap. Tracked as a running total.
    per_window_total: f64,
}

impl ScoreAccumulator {
    /// Build from a full initial scoring pass.
    pub fn init(layout: &Layout, corpus: &dyn CorpusSource, pipeline: &Pipeline) -> Self {
        let scopes = pipeline.scopes();

        let bigram_index = if scopes.contains(&Scope::Bigram) {
            Some(build_bigram_index(corpus))
        } else {
            None
        };
        let trigram_index = if scopes.contains(&Scope::Trigram) {
            Some(build_trigram_index(corpus))
        } else {
            None
        };
        let mut ngram_indexes = HashMap::new();
        let mut skipgram_indexes = HashMap::new();
        for scope in &scopes {
            match scope {
                Scope::Ngram(n) => {
                    ngram_indexes.insert(*n, build_ngram_index(corpus, *n));
                }
                Scope::Skipgram(gap) => {
                    skipgram_indexes.insert(*gap, build_skipgram_index(corpus, *gap));
                }
                _ => {}
            }
        }

        // Per-char and per-finger load.
        let mut char_load: HashMap<char, f64> = HashMap::new();
        let mut finger_load: HashMap<Finger, f64> = HashMap::new();
        for (ch, freq) in corpus.iter_chars() {
            if let Some(k) = layout.position(ch) {
                *char_load.entry(ch).or_insert(0.0) += freq;
                *finger_load.entry(k.finger).or_insert(0.0) += freq;
            }
        }

        // Initial full pass — sum every window's contribution.
        let mut per_window_total = 0.0;
        if let Some(idx) = &bigram_index {
            per_window_total += sum_index_contributions(idx, Scope::Bigram, layout, pipeline);
        }
        if let Some(idx) = &trigram_index {
            per_window_total += sum_index_contributions(idx, Scope::Trigram, layout, pipeline);
        }
        for (n, idx) in &ngram_indexes {
            per_window_total +=
                sum_index_contributions(idx, Scope::Ngram(*n), layout, pipeline);
        }
        for (gap, idx) in &skipgram_indexes {
            per_window_total +=
                sum_index_contributions(idx, Scope::Skipgram(*gap), layout, pipeline);
        }

        // Aggregate pass.
        let aggregate_cost = aggregate_contribution(pipeline, layout, corpus, &char_load, &finger_load);

        // Unigram-scope analyzers contribute to the total but their
        // output depends only on char load, which the aggregate
        // context already exposes. Run them once at init and roll
        // their cost into aggregate_cost so swaps don't need a
        // separate unigram pass.
        let unigram_cost = unigram_contribution(pipeline, layout, corpus);

        let total = per_window_total + aggregate_cost + unigram_cost;

        Self {
            total,
            bigram_index,
            trigram_index,
            ngram_indexes,
            skipgram_indexes,
            char_load,
            finger_load,
            aggregate_cost: aggregate_cost + unigram_cost,
            per_window_total,
        }
    }

    /// Score the layout would have if chars `a` and `b` swapped,
    /// without mutating state.
    pub fn swap_delta(
        &self,
        layout: &Layout,
        a: char,
        b: char,
        corpus: &dyn CorpusSource,
        pipeline: &Pipeline,
    ) -> f64 {
        if a == b {
            return self.total;
        }
        let swapped = swapped_layout(layout, a, b);

        let mut before = 0.0;
        let mut after = 0.0;
        let affected = affected_chars(a, b);

        if let Some(idx) = &self.bigram_index {
            before += sum_affected(idx, &affected, Scope::Bigram, layout, pipeline);
            after += sum_affected(idx, &affected, Scope::Bigram, &swapped, pipeline);
        }
        if let Some(idx) = &self.trigram_index {
            before += sum_affected(idx, &affected, Scope::Trigram, layout, pipeline);
            after += sum_affected(idx, &affected, Scope::Trigram, &swapped, pipeline);
        }
        for (n, idx) in &self.ngram_indexes {
            before += sum_affected(idx, &affected, Scope::Ngram(*n), layout, pipeline);
            after += sum_affected(idx, &affected, Scope::Ngram(*n), &swapped, pipeline);
        }
        for (gap, idx) in &self.skipgram_indexes {
            before += sum_affected(idx, &affected, Scope::Skipgram(*gap), layout, pipeline);
            after += sum_affected(idx, &affected, Scope::Skipgram(*gap), &swapped, pipeline);
        }

        // Aggregate: only changes if the swap moves chars between
        // fingers. Otherwise keep the cached value.
        let aggregate_after = match (layout.position(a), layout.position(b)) {
            (Some(ka), Some(kb)) if ka.finger != kb.finger => {
                let next_finger_load = swap_finger_load(&self.finger_load, corpus, a, b, ka, kb);
                aggregate_contribution(
                    pipeline,
                    &swapped,
                    corpus,
                    &self.char_load,
                    &next_finger_load,
                ) + unigram_contribution(pipeline, &swapped, corpus)
            }
            _ => self.aggregate_cost,
        };

        self.per_window_total - before + after + aggregate_after
    }

    /// Apply a swap to the accumulator. Caller must have mutated
    /// the layout already.
    pub fn commit_swap(
        &mut self,
        layout_after_swap: &Layout,
        a: char,
        b: char,
        corpus: &dyn CorpusSource,
        pipeline: &Pipeline,
    ) {
        if a == b {
            return;
        }
        // To compute "before", we temporarily un-swap to get the
        // pre-swap positions. Cheap clone of the positions map.
        let pre_swap = swapped_layout(layout_after_swap, a, b);
        let affected = affected_chars(a, b);

        let mut before = 0.0;
        let mut after = 0.0;
        if let Some(idx) = &self.bigram_index {
            before += sum_affected(idx, &affected, Scope::Bigram, &pre_swap, pipeline);
            after += sum_affected(idx, &affected, Scope::Bigram, layout_after_swap, pipeline);
        }
        if let Some(idx) = &self.trigram_index {
            before += sum_affected(idx, &affected, Scope::Trigram, &pre_swap, pipeline);
            after += sum_affected(idx, &affected, Scope::Trigram, layout_after_swap, pipeline);
        }
        for (n, idx) in &self.ngram_indexes {
            before += sum_affected(idx, &affected, Scope::Ngram(*n), &pre_swap, pipeline);
            after += sum_affected(idx, &affected, Scope::Ngram(*n), layout_after_swap, pipeline);
        }
        for (gap, idx) in &self.skipgram_indexes {
            before += sum_affected(idx, &affected, Scope::Skipgram(*gap), &pre_swap, pipeline);
            after += sum_affected(
                idx,
                &affected,
                Scope::Skipgram(*gap),
                layout_after_swap,
                pipeline,
            );
        }
        self.per_window_total = self.per_window_total - before + after;

        // Update finger_load if the swap crossed fingers. Args to
        // swap_finger_load are "previous finger = kb.finger for a,
        // ka.finger for b" since post-swap a is on ka, so pre-swap
        // a was on kb.
        if let (Some(ka), Some(kb)) = (
            layout_after_swap.position(a),
            layout_after_swap.position(b),
        ) && ka.finger != kb.finger
        {
            self.finger_load = swap_finger_load(&self.finger_load, corpus, a, b, kb, ka);
        }
        let new_aggregate = aggregate_contribution(
            pipeline,
            layout_after_swap,
            corpus,
            &self.char_load,
            &self.finger_load,
        ) + unigram_contribution(pipeline, layout_after_swap, corpus);
        self.aggregate_cost = new_aggregate;

        self.total = self.per_window_total + self.aggregate_cost;
    }
}

fn affected_chars(a: char, b: char) -> HashSet<char> {
    let mut s = HashSet::new();
    s.insert(a);
    s.insert(b);
    s
}

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

fn build_bigram_index(corpus: &dyn CorpusSource) -> WindowIndex {
    // Collect bigrams into a sorted Vec first so window insertion
    // order is deterministic across runs. iter_bigrams uses the
    // underlying corpus's HashMap iteration order, which isn't
    // stable — and the initial full-pass sum is order-dependent
    // at the floating-point level.
    let mut bigrams: Vec<((char, char), f64)> = corpus.iter_bigrams().collect();
    bigrams.sort_by(|a, b| a.0.cmp(&b.0));

    let mut windows = Vec::with_capacity(bigrams.len());
    let mut by_char: HashMap<char, Vec<usize>> = HashMap::new();
    for ((a, b), freq) in bigrams {
        let idx = windows.len();
        windows.push((vec![a, b], freq));
        by_char.entry(a).or_default().push(idx);
        if b != a {
            by_char.entry(b).or_default().push(idx);
        }
    }
    WindowIndex { windows, by_char }
}

fn build_trigram_index(corpus: &dyn CorpusSource) -> WindowIndex {
    let mut trigrams: Vec<((char, char, char), f64)> = corpus.iter_trigrams().collect();
    trigrams.sort_by(|a, b| a.0.cmp(&b.0));

    let mut windows = Vec::with_capacity(trigrams.len());
    let mut by_char: HashMap<char, Vec<usize>> = HashMap::new();
    for ((a, b, c), freq) in trigrams {
        let idx = windows.len();
        windows.push((vec![a, b, c], freq));
        by_char.entry(a).or_default().push(idx);
        if b != a {
            by_char.entry(b).or_default().push(idx);
        }
        if c != a && c != b {
            by_char.entry(c).or_default().push(idx);
        }
    }
    WindowIndex { windows, by_char }
}

fn build_skipgram_index(corpus: &dyn CorpusSource, gap: usize) -> WindowIndex {
    let mut pairs: Vec<((char, char), f64)> = corpus.iter_skipgrams(gap).collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut windows = Vec::with_capacity(pairs.len());
    let mut by_char: HashMap<char, Vec<usize>> = HashMap::new();
    for ((a, b), freq) in pairs {
        let idx = windows.len();
        windows.push((vec![a, b], freq));
        by_char.entry(a).or_default().push(idx);
        if b != a {
            by_char.entry(b).or_default().push(idx);
        }
    }
    WindowIndex { windows, by_char }
}

fn build_ngram_index(corpus: &dyn CorpusSource, n: usize) -> WindowIndex {
    let mut ngrams: Vec<(Vec<char>, f64)> = corpus
        .iter_ngrams(n)
        .filter(|(chars, _)| chars.len() == n)
        .collect();
    ngrams.sort_by(|a, b| a.0.cmp(&b.0));

    let mut windows = Vec::with_capacity(ngrams.len());
    let mut by_char: HashMap<char, Vec<usize>> = HashMap::new();
    for (chars, freq) in ngrams {
        let idx = windows.len();
        let mut seen: Vec<char> = chars.clone();
        seen.sort_unstable();
        seen.dedup();
        windows.push((chars, freq));
        for ch in seen {
            by_char.entry(ch).or_default().push(idx);
        }
    }
    WindowIndex { windows, by_char }
}

fn sum_index_contributions(
    idx: &WindowIndex,
    scope: Scope,
    layout: &Layout,
    pipeline: &Pipeline,
) -> f64 {
    let mut sum = 0.0;
    for (chars, freq) in &idx.windows {
        sum += window_contribution(chars, *freq, scope, layout, pipeline);
    }
    sum
}

fn sum_affected(
    idx: &WindowIndex,
    affected: &HashSet<char>,
    scope: Scope,
    layout: &Layout,
    pipeline: &Pipeline,
) -> f64 {
    // Collect window indices touching any affected char. Use a set
    // to dedupe windows containing both chars, then sort for
    // deterministic summation — floating-point addition isn't
    // associative, and accumulated order-dependent error corrupts
    // seeded SA reproducibility.
    let mut seen: HashSet<usize> = HashSet::new();
    for ch in affected {
        if let Some(ids) = idx.by_char.get(ch) {
            seen.extend(ids);
        }
    }
    let mut order: Vec<usize> = seen.into_iter().collect();
    order.sort_unstable();
    let mut sum = 0.0;
    for i in order {
        let (chars, freq) = &idx.windows[i];
        sum += window_contribution(chars, *freq, scope, layout, pipeline);
    }
    sum
}

fn window_contribution(
    chars: &[char],
    freq: f64,
    scope: Scope,
    layout: &Layout,
    pipeline: &Pipeline,
) -> f64 {
    // Resolve keys; if any char isn't on the layout, the window
    // can't contribute (analyzers only see windows with every key
    // resolved).
    let mut keys: Vec<&Key> = Vec::with_capacity(chars.len());
    for &ch in chars {
        match layout.position(ch) {
            Some(k) => keys.push(k),
            None => return 0.0,
        }
    }
    let props = build_props(&keys);
    let window = Window {
        chars,
        keys: &keys,
        freq,
        props: &props,
    };
    let mut sum = 0.0;
    for analyzer in pipeline.for_scope(scope) {
        for hit in analyzer.evaluate(&window) {
            sum += hit.cost;
        }
    }
    sum
}

fn build_props(keys: &[&Key]) -> WindowProps {
    let mut same_hand_pairs = Vec::with_capacity(keys.len().saturating_sub(1));
    for pair in keys.windows(2) {
        same_hand_pairs.push(pair[0].finger.same_hand(pair[1].finger));
    }
    let all_same_hand = same_hand_pairs.iter().all(|&x| x);
    let finger_columns = keys.iter().map(|k| k.finger.column()).collect();
    let rows = keys.iter().map(|k| k.row).collect();
    WindowProps {
        same_hand_pairs,
        all_same_hand,
        finger_columns,
        rows,
    }
}

fn aggregate_contribution(
    pipeline: &Pipeline,
    layout: &Layout,
    corpus: &dyn CorpusSource,
    char_load: &HashMap<char, f64>,
    finger_load: &HashMap<Finger, f64>,
) -> f64 {
    let ctx = AggregateContext {
        layout,
        corpus_name: corpus.name(),
        char_load,
        finger_load,
    };
    let mut sum = 0.0;
    for analyzer in pipeline.for_scope(Scope::Aggregate) {
        for hit in analyzer.evaluate_aggregate(&ctx) {
            sum += hit.cost;
        }
    }
    sum
}

fn unigram_contribution(pipeline: &Pipeline, layout: &Layout, corpus: &dyn CorpusSource) -> f64 {
    // Sort by char so summation is deterministic. iter_chars uses
    // underlying HashMap order, which differs across runs.
    let mut chars_sorted: Vec<(char, f64)> = corpus.iter_chars().collect();
    chars_sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let mut sum = 0.0;
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
            for hit in analyzer.evaluate(&window) {
                sum += hit.cost;
            }
        }
    }
    sum
}

/// Compute the post-swap `finger_load` map without mutating the
/// input. `ka` is the KEY that char `a` currently sits on;
/// symmetrically `kb` for `b`. After the swap, `a` moves to `kb.finger`
/// and `b` moves to `ka.finger`.
fn swap_finger_load(
    current: &HashMap<Finger, f64>,
    corpus: &dyn CorpusSource,
    a: char,
    b: char,
    ka: &Key,
    kb: &Key,
) -> HashMap<Finger, f64> {
    let mut next = current.clone();
    let fa = corpus.char_freq(a);
    let fb = corpus.char_freq(b);
    // Remove `a` from its old finger, add to new.
    *next.entry(ka.finger).or_insert(0.0) -= fa;
    *next.entry(kb.finger).or_insert(0.0) += fa;
    // Remove `b` from its old finger, add to new.
    *next.entry(kb.finger).or_insert(0.0) -= fb;
    *next.entry(ka.finger).or_insert(0.0) += fb;
    next
}
