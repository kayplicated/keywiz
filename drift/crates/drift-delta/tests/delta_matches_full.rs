//! Delta scoring must agree with full rescoring.
//!
//! For any candidate swap, `ScoreAccumulator::swap_delta` has to
//! produce the same total that a full `drift_score::run` would on
//! the swapped layout. We exercise both same-finger and cross-finger
//! swaps, and both the drifter and neutral presets, so regressions
//! in either codepath surface immediately.

use std::path::PathBuf;

use drift_analyzer::Registry;

fn repo_root() -> PathBuf {
    // Cargo sets CARGO_MANIFEST_DIR to drift-delta's crate root.
    // Repo root is three levels up (drift/crates/drift-delta → drift/crates → drift → repo).
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn load_pieces(
    preset: &str,
    keyboard_path: &str,
    layout_path: &str,
) -> (
    drift_analyzer::Pipeline,
    drift_core::Keyboard,
    drift_core::Layout,
    drift_corpus::MemoryCorpus,
) {
    let root = repo_root();
    let mut registry = Registry::new();
    drift_analyzers::register_all(&mut registry);

    let config = drift_config::load_preset(preset).expect("preset load");
    let pipeline = drift_config::build_pipeline(&config, &registry).expect("pipeline build");

    let keyboard = drift_keyboard::load_keyboard(&root.join(keyboard_path)).expect("keyboard");
    let layout =
        drift_keyboard::load_layout(&root.join(layout_path), &keyboard).expect("layout");
    let mut corpus = drift_corpus::load(&config.corpus_path).expect("corpus");

    // Mirror the CLI's derivation step so delta testing exercises
    // the real per-preset pipeline, including any Scope::Ngram(n>=4)
    // analyzers. Without this, n>=4 scopes see an empty iterator
    // and the analyzer contributes nothing — the equality holds
    // vacuously.
    if let Some(max_n) = pipeline
        .scopes()
        .into_iter()
        .filter_map(|s| match s {
            drift_core::Scope::Ngram(n) if n >= 4 => Some(n),
            _ => None,
        })
        .max()
    {
        corpus.ensure_ngrams(max_n).expect("ensure_ngrams");
    }
    (pipeline, keyboard, layout, corpus)
}

fn swap_in_place(layout: &drift_core::Layout, a: char, b: char) -> drift_core::Layout {
    let mut next = layout.clone();
    let (Some(ka), Some(kb)) = (next.positions.remove(&a), next.positions.remove(&b)) else {
        panic!("chars {:?} or {:?} missing from layout", a, b);
    };
    next.positions.insert(a, kb);
    next.positions.insert(b, ka);
    next
}

fn assert_delta_matches(preset: &str, pairs: &[(char, char)]) {
    let (pipeline, keyboard, layout, corpus) = load_pieces(
        preset,
        "keyboards/halcyon_elora_v2.json",
        "layouts/drifter.json",
    );

    let accumulator = drift_delta::ScoreAccumulator::init(&layout, &corpus, &pipeline);
    let full_before = drift_score::run(&pipeline, &layout, &keyboard, &corpus).total;
    assert!(
        (accumulator.total - full_before).abs() < 1e-9,
        "accumulator.total {} disagrees with full rescore {}",
        accumulator.total,
        full_before
    );

    for (a, b) in pairs {
        let predicted = accumulator.swap_delta(&layout, *a, *b, &corpus, &pipeline);
        let swapped = swap_in_place(&layout, *a, *b);
        let actual = drift_score::run(&pipeline, &swapped, &keyboard, &corpus).total;
        assert!(
            (predicted - actual).abs() < 1e-9,
            "[{preset}] swap {a}{b}: predicted {predicted} vs full {actual} (Δ {:.3e})",
            predicted - actual
        );
    }
}

#[test]
fn delta_matches_full_drifter_preset() {
    assert_delta_matches(
        "drifter",
        &[('e', 'a'), ('r', 't'), ('n', 'p'), ('q', 'z'), ('e', 'h'), ('k', 'f')],
    );
}

#[test]
fn delta_matches_full_neutral_preset() {
    assert_delta_matches(
        "neutral",
        &[('e', 'a'), ('r', 't'), ('n', 'p'), ('q', 'z')],
    );
}

#[test]
fn delta_matches_full_extension_preset() {
    assert_delta_matches(
        "extension",
        &[('e', 'a'), ('r', 't'), ('n', 'p'), ('q', 'z'), ('e', 'h')],
    );
}

#[test]
fn commit_swap_leaves_accumulator_consistent() {
    // After commit_swap, the accumulator's cached total must match
    // a fresh full rescore on the post-swap layout. Otherwise SA
    // runs will drift away from true scores over many iterations.
    let (pipeline, keyboard, layout, corpus) = load_pieces(
        "drifter",
        "keyboards/halcyon_elora_v2.json",
        "layouts/drifter.json",
    );

    let swaps: &[(char, char)] = &[('e', 'a'), ('r', 'n'), ('q', 'z')];
    let mut acc = drift_delta::ScoreAccumulator::init(&layout, &corpus, &pipeline);
    let mut live_layout = layout.clone();

    for (a, b) in swaps {
        live_layout = swap_in_place(&live_layout, *a, *b);
        acc.commit_swap(&live_layout, *a, *b, &corpus, &pipeline);

        let full = drift_score::run(&pipeline, &live_layout, &keyboard, &corpus).total;
        assert!(
            (acc.total - full).abs() < 1e-9,
            "after commit_swap {}{}: acc.total {} vs full {}",
            a,
            b,
            acc.total,
            full
        );
    }
}
