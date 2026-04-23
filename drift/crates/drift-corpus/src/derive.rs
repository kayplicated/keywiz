//! N-gram derivation for `n ≥ 4` via the Markov chain rule.
//!
//! When a corpus stores frequencies only up to trigrams (the oxey
//! format), higher-order n-grams can be *approximated* by chaining
//! overlapping lower-order windows:
//!
//! ```text
//! P(c1..cn) ≈ P(c1..cn-1) × P(c2..cn) / P(c2..cn-1)
//! ```
//!
//! The approximation assumes conditional independence of position
//! `n` given positions `2..n-1` — which doesn't strictly hold in
//! English, but is the same assumption every Markov language model
//! makes, and is good enough for layout-scoring purposes.
//!
//! This module owns the math. Storage and the `ensure_ngrams`
//! cache entrypoint live in [`crate::memory`].
//!
//! Performance note: the naive enumeration is `|prev| × |chars|`
//! candidates per step, filtered by the overlap constraint. For an
//! oxey-format English corpus (~50 real chars, ~10k trigrams), each
//! derivation step produces ~100k entries at n=4 and scales
//! sub-linearly because the overlap becomes more restrictive.

use std::collections::HashMap;

/// Entries below this joint probability are dropped from the
/// derived table. Keeps higher-n tables from blowing up with near-
/// zero noise while preserving every entry that matters to scoring.
const PRUNE_THRESHOLD: f64 = 1e-9;

/// Derive the 4-gram table from a corpus's trigrams and bigrams.
///
/// `P(abcd) ≈ P(abc) × P(bcd) / P(bc)`
///
/// Entries with `P(bc) = 0` or with `P(abc) × P(bcd) = 0` collapse
/// to zero and are pruned.
pub fn derive_4grams(
    trigrams: &HashMap<(char, char, char), f64>,
    bigrams: &HashMap<(char, char), f64>,
) -> HashMap<Vec<char>, f64> {
    // Index trigrams by their last two chars so for each `(b, c)`
    // we can cheaply enumerate the trigrams that end there
    // (left-extensions `abc`) and the trigrams that start there
    // (right-extensions `bcd`).
    let mut ending_at: HashMap<(char, char), Vec<(char, f64)>> = HashMap::new();
    let mut starting_at: HashMap<(char, char), Vec<(char, f64)>> = HashMap::new();
    for (&(a, b, c), &p) in trigrams {
        ending_at.entry((b, c)).or_default().push((a, p));
        starting_at.entry((a, b)).or_default().push((c, p));
    }

    let mut out: HashMap<Vec<char>, f64> = HashMap::new();
    for (&(b, c), lefts) in &ending_at {
        let Some(rights) = starting_at.get(&(b, c)) else {
            continue;
        };
        let Some(&p_bc) = bigrams.get(&(b, c)) else {
            continue;
        };
        if p_bc <= 0.0 {
            continue;
        }
        for &(a, p_abc) in lefts {
            for &(d, p_bcd) in rights {
                let joint = p_abc * p_bcd / p_bc;
                if joint >= PRUNE_THRESHOLD {
                    out.insert(vec![a, b, c, d], joint);
                }
            }
        }
    }
    out
}

/// Derive the `n`-gram table from the corpus's `(n-1)`-gram table
/// and `(n-2)`-gram table.
///
/// `P(c1..cn) ≈ P(c1..cn-1) × P(c2..cn) / P(c2..cn-1)`
///
/// `prev` is the `(n-1)`-gram table; `bridge` is the `(n-2)`-gram
/// table that supplies the denominator. `n` must be ≥ 4.
///
/// Returns an empty map if `prev` is empty (nothing to extend).
pub fn derive_ngrams(
    prev: &HashMap<Vec<char>, f64>,
    bridge: &HashMap<Vec<char>, f64>,
    n: usize,
) -> HashMap<Vec<char>, f64> {
    assert!(n >= 4, "derive_ngrams requires n >= 4, got {n}");

    // Index `prev` by its trailing `(n-2)` chars (for left-extensions
    // c1..cn-1 whose suffix is c2..cn-1) and by its leading `(n-2)`
    // chars (for right-extensions c2..cn whose prefix is c2..cn-1).
    let mut ending_at: HashMap<Vec<char>, Vec<(char, f64)>> = HashMap::new();
    let mut starting_at: HashMap<Vec<char>, Vec<(char, f64)>> = HashMap::new();
    for (window, &p) in prev {
        if window.len() != n - 1 {
            continue;
        }
        let tail = window[1..].to_vec();
        let head = window[..n - 2].to_vec();
        ending_at.entry(tail).or_default().push((window[0], p));
        starting_at
            .entry(head)
            .or_default()
            .push((window[n - 2], p));
    }

    let mut out: HashMap<Vec<char>, f64> = HashMap::new();
    for (bridge_key, lefts) in &ending_at {
        let Some(rights) = starting_at.get(bridge_key) else {
            continue;
        };
        let Some(&p_bridge) = bridge.get(bridge_key) else {
            continue;
        };
        if p_bridge <= 0.0 {
            continue;
        }
        for &(first, p_left) in lefts {
            for &(last, p_right) in rights {
                let joint = p_left * p_right / p_bridge;
                if joint < PRUNE_THRESHOLD {
                    continue;
                }
                let mut key = Vec::with_capacity(n);
                key.push(first);
                key.extend_from_slice(bridge_key);
                key.push(last);
                out.insert(key, joint);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tiny deterministic corpus over {a, b}. Hand-specified so we
    /// can verify the math at the entry level.
    fn tiny_corpus() -> (
        HashMap<(char, char), f64>,
        HashMap<(char, char, char), f64>,
    ) {
        // Chars: 'a' 0.5, 'b' 0.5
        // Bigrams: aa 0.3, ab 0.2, ba 0.2, bb 0.3
        let mut bigrams = HashMap::new();
        bigrams.insert(('a', 'a'), 0.3);
        bigrams.insert(('a', 'b'), 0.2);
        bigrams.insert(('b', 'a'), 0.2);
        bigrams.insert(('b', 'b'), 0.3);

        // Trigrams: distribute so aXb + bXa conservation holds
        //   aaa 0.2, aab 0.1, aba 0.1, abb 0.1
        //   baa 0.1, bab 0.1, bba 0.1, bbb 0.2
        let mut trigrams = HashMap::new();
        trigrams.insert(('a', 'a', 'a'), 0.2);
        trigrams.insert(('a', 'a', 'b'), 0.1);
        trigrams.insert(('a', 'b', 'a'), 0.1);
        trigrams.insert(('a', 'b', 'b'), 0.1);
        trigrams.insert(('b', 'a', 'a'), 0.1);
        trigrams.insert(('b', 'a', 'b'), 0.1);
        trigrams.insert(('b', 'b', 'a'), 0.1);
        trigrams.insert(('b', 'b', 'b'), 0.2);
        (bigrams, trigrams)
    }

    #[test]
    fn four_gram_from_tiny_corpus_matches_formula() {
        let (bigrams, trigrams) = tiny_corpus();
        let out = derive_4grams(&trigrams, &bigrams);

        // Expect 16 4-grams over {a, b}, all should have non-zero
        // probability given our table has no zero entries.
        assert_eq!(out.len(), 16);

        // Spot-check: P(aaaa) = P(aaa) * P(aaa) / P(aa)
        //                     = 0.2 * 0.2 / 0.3 ≈ 0.133333
        let p_aaaa = out[&vec!['a', 'a', 'a', 'a']];
        assert!((p_aaaa - (0.2 * 0.2 / 0.3)).abs() < 1e-12);

        // P(aabb) = P(aab) * P(abb) / P(ab)
        //         = 0.1 * 0.1 / 0.2 = 0.05
        let p_aabb = out[&vec!['a', 'a', 'b', 'b']];
        assert!((p_aabb - 0.05).abs() < 1e-12);
    }

    #[test]
    fn missing_bridge_bigram_drops_entry_while_others_survive() {
        // Two overlap bridges: `aa` (present) and `ab` (omitted).
        // 4-grams that need `P(ab)` must drop; 4-grams that need
        // `P(aa)` must survive. This catches both "didn't drop the
        // right one" and "silently returned empty" regressions.
        let mut bigrams = HashMap::new();
        bigrams.insert(('a', 'a'), 0.5);
        // Deliberately omit ('a', 'b').

        let mut trigrams = HashMap::new();
        // Path through bridge `aa` — should survive:
        //   aaaa = P(aaa) × P(aaa) / P(aa)
        trigrams.insert(('a', 'a', 'a'), 0.2);
        // Paths through bridge `ab` — should drop (no P(ab)):
        trigrams.insert(('a', 'a', 'b'), 0.1);
        trigrams.insert(('a', 'b', 'a'), 0.1);

        let out = derive_4grams(&trigrams, &bigrams);
        assert!(
            out.contains_key(&vec!['a', 'a', 'a', 'a']),
            "4-gram via present bridge aa should survive: {out:?}"
        );
        assert!(
            !out.contains_key(&vec!['a', 'a', 'b', 'a']),
            "4-gram via absent bridge ab should drop"
        );
    }

    #[test]
    fn prune_threshold_drops_below_and_keeps_above() {
        // Symmetric check: an entry just above the cutoff survives,
        // an entry just below drops. Catches "threshold reversed" or
        // "threshold moved" regressions.
        //
        // P(abcd) = P(abc) × P(bcd) / P(bc). With P(bc) = 1.0, the
        // joint equals the product of the two trigrams.
        let mut bigrams = HashMap::new();
        bigrams.insert(('a', 'a'), 1.0);
        bigrams.insert(('b', 'b'), 1.0);

        let mut trigrams = HashMap::new();
        // Path through `aa` bridge — joint = sqrt(above) × sqrt(above)
        // chosen to land at 1.1e-9, above PRUNE_THRESHOLD (1e-9).
        let above = 1.1e-9_f64.sqrt();
        trigrams.insert(('a', 'a', 'a'), above);
        // Path through `bb` bridge — joint = 0.9e-9, below threshold.
        let below = 0.9e-9_f64.sqrt();
        trigrams.insert(('b', 'b', 'b'), below);

        let out = derive_4grams(&trigrams, &bigrams);
        assert!(
            out.contains_key(&vec!['a', 'a', 'a', 'a']),
            "entry at {:e} (above 1e-9) should survive",
            1.1e-9
        );
        assert!(
            !out.contains_key(&vec!['b', 'b', 'b', 'b']),
            "entry at {:e} (below 1e-9) should be pruned",
            0.9e-9
        );
    }

    #[test]
    fn five_gram_chains_via_derive_ngrams() {
        let (bigrams, trigrams) = tiny_corpus();
        let four = derive_4grams(&trigrams, &bigrams);

        // Bridge for n=5 is the trigram table, expressed as
        // Vec<char> for the generic derive_ngrams signature.
        let trigram_vec: HashMap<Vec<char>, f64> = trigrams
            .iter()
            .map(|(&(a, b, c), &p)| (vec![a, b, c], p))
            .collect();

        let five = derive_ngrams(&four, &trigram_vec, 5);

        // P(aaaaa) = P(aaaa) * P(aaaa) / P(aaa)
        //          = (0.2 * 0.2 / 0.3)^2 / 0.2
        //          ≈ 0.01777... / 0.2
        //          ≈ 0.08889
        let p4_aaaa = 0.2 * 0.2 / 0.3;
        let expect = p4_aaaa * p4_aaaa / 0.2;
        let got = five[&vec!['a', 'a', 'a', 'a', 'a']];
        assert!((got - expect).abs() < 1e-12, "got {got}, want {expect}");
    }

    #[test]
    #[should_panic(expected = "derive_ngrams requires n >= 4")]
    fn rejects_n_below_4() {
        derive_ngrams(&HashMap::new(), &HashMap::new(), 3);
    }
}
