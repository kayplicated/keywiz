//! In-memory `CorpusSource` — holds the whole corpus in `HashMap`s.
//!
//! This is the default implementation drift-corpus produces when
//! loading an oxey JSON file. Alternative implementations
//! (streaming, memory-mapped) can live alongside without changing
//! the trait or any analyzer.

use std::collections::HashMap;

use anyhow::{Result, bail};
use drift_core::CorpusSource;

use crate::derive;

/// In-memory frequency store. All lookups are `HashMap::get`.
pub struct MemoryCorpus {
    pub name: String,
    pub chars: HashMap<char, f64>,
    pub bigrams: HashMap<(char, char), f64>,
    pub trigrams: HashMap<(char, char, char), f64>,
    /// Higher-order n-grams keyed by length. Sparse; a length
    /// appears in the map only if it's been loaded or derived.
    pub ngrams: HashMap<usize, HashMap<Vec<char>, f64>>,
    /// Skipgrams keyed by gap. A gap of 1 corresponds to the oxey
    /// `skipgrams` field, 2 to `skipgrams2`, etc. Missing gaps
    /// simply produce an empty iterator from `iter_skipgrams`.
    pub skipgrams: HashMap<usize, HashMap<(char, char), f64>>,
}

impl MemoryCorpus {
    /// Ensure the `n`-gram table is populated, deriving it from
    /// lower-order frequencies if needed.
    ///
    /// Idempotent. `n <= 3` is a no-op (trigrams and below live in
    /// their own fields). `n >= 4` recursively fills every level
    /// between the current `max_ngram_length()` and `n`.
    ///
    /// Derivation uses the Markov chain rule — see [`crate::derive`].
    /// Frequencies become approximations once `n >= 4`; the error
    /// is systemic rather than random, so rankings between layouts
    /// remain meaningful.
    pub fn ensure_ngrams(&mut self, n: usize) -> Result<()> {
        if n <= 3 {
            return Ok(());
        }

        // Derive 4 from (trigrams, bigrams) once.
        if !self.ngrams.contains_key(&4) {
            let derived = derive::derive_4grams(&self.trigrams, &self.bigrams);
            self.ngrams.insert(4, derived);
        }

        // For n >= 5, chain: each level uses the level below plus
        // the level two below as the bridge.
        for target in 5..=n {
            if self.ngrams.contains_key(&target) {
                continue;
            }
            let Some(prev) = self.ngrams.get(&(target - 1)) else {
                bail!("ensure_ngrams: missing prerequisite level {}", target - 1);
            };
            // `bridge` is the (target-2)-gram table. For target=5
            // that's the trigram table, which lives outside
            // `self.ngrams` — materialize a Vec<char>-keyed view.
            let bridge_owned;
            let bridge: &HashMap<Vec<char>, f64> = if target == 5 {
                bridge_owned = self
                    .trigrams
                    .iter()
                    .map(|(&(a, b, c), &p)| (vec![a, b, c], p))
                    .collect();
                &bridge_owned
            } else {
                self.ngrams
                    .get(&(target - 2))
                    .expect("previous derivation step populated this level")
            };
            let derived = derive::derive_ngrams(prev, bridge, target);
            self.ngrams.insert(target, derived);
        }
        Ok(())
    }
}

impl CorpusSource for MemoryCorpus {
    fn name(&self) -> &str {
        &self.name
    }

    fn char_freq(&self, c: char) -> f64 {
        self.chars.get(&c).copied().unwrap_or(0.0)
    }

    fn bigram_freq(&self, a: char, b: char) -> f64 {
        self.bigrams.get(&(a, b)).copied().unwrap_or(0.0)
    }

    fn trigram_freq(&self, a: char, b: char, c: char) -> f64 {
        self.trigrams.get(&(a, b, c)).copied().unwrap_or(0.0)
    }

    fn ngram_freq(&self, chars: &[char]) -> f64 {
        match chars.len() {
            1 => self.char_freq(chars[0]),
            2 => self.bigram_freq(chars[0], chars[1]),
            3 => self.trigram_freq(chars[0], chars[1], chars[2]),
            n => self
                .ngrams
                .get(&n)
                .and_then(|m| m.get(chars))
                .copied()
                .unwrap_or(0.0),
        }
    }

    fn iter_chars<'a>(&'a self) -> Box<dyn Iterator<Item = (char, f64)> + 'a> {
        Box::new(self.chars.iter().map(|(&c, &f)| (c, f)))
    }

    fn iter_bigrams<'a>(&'a self) -> Box<dyn Iterator<Item = ((char, char), f64)> + 'a> {
        Box::new(self.bigrams.iter().map(|(&p, &f)| (p, f)))
    }

    fn iter_trigrams<'a>(&'a self) -> Box<dyn Iterator<Item = ((char, char, char), f64)> + 'a> {
        Box::new(self.trigrams.iter().map(|(&t, &f)| (t, f)))
    }

    fn iter_ngrams<'a>(&'a self, n: usize) -> Box<dyn Iterator<Item = (Vec<char>, f64)> + 'a> {
        match n {
            1 => Box::new(self.chars.iter().map(|(&c, &f)| (vec![c], f))),
            2 => Box::new(
                self.bigrams
                    .iter()
                    .map(|(&(a, b), &f)| (vec![a, b], f)),
            ),
            3 => Box::new(
                self.trigrams
                    .iter()
                    .map(|(&(a, b, c), &f)| (vec![a, b, c], f)),
            ),
            other => match self.ngrams.get(&other) {
                Some(map) => Box::new(map.iter().map(|(k, &f)| (k.clone(), f))),
                None => Box::new(std::iter::empty()),
            },
        }
    }

    fn iter_skipgrams<'a>(
        &'a self,
        gap: usize,
    ) -> Box<dyn Iterator<Item = ((char, char), f64)> + 'a> {
        match self.skipgrams.get(&gap) {
            Some(map) => Box::new(map.iter().map(|(&p, &f)| (p, f))),
            None => Box::new(std::iter::empty()),
        }
    }

    fn max_ngram_length(&self) -> usize {
        let extras = self.ngrams.keys().copied().max().unwrap_or(0);
        std::cmp::max(3, extras)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_corpus() -> MemoryCorpus {
        // Same tiny {a, b} corpus used in derive tests so we can
        // cross-check. Bigrams sum to 1.0, trigrams sum to 1.0.
        let mut chars = HashMap::new();
        chars.insert('a', 0.5);
        chars.insert('b', 0.5);

        let mut bigrams = HashMap::new();
        bigrams.insert(('a', 'a'), 0.3);
        bigrams.insert(('a', 'b'), 0.2);
        bigrams.insert(('b', 'a'), 0.2);
        bigrams.insert(('b', 'b'), 0.3);

        let mut trigrams = HashMap::new();
        trigrams.insert(('a', 'a', 'a'), 0.2);
        trigrams.insert(('a', 'a', 'b'), 0.1);
        trigrams.insert(('a', 'b', 'a'), 0.1);
        trigrams.insert(('a', 'b', 'b'), 0.1);
        trigrams.insert(('b', 'a', 'a'), 0.1);
        trigrams.insert(('b', 'a', 'b'), 0.1);
        trigrams.insert(('b', 'b', 'a'), 0.1);
        trigrams.insert(('b', 'b', 'b'), 0.2);

        MemoryCorpus {
            name: "tiny".into(),
            chars,
            bigrams,
            trigrams,
            ngrams: HashMap::new(),
            skipgrams: HashMap::new(),
        }
    }

    #[test]
    fn ensure_ngrams_fills_level_4() {
        let mut c = tiny_corpus();
        assert_eq!(c.max_ngram_length(), 3);
        c.ensure_ngrams(4).unwrap();
        assert_eq!(c.max_ngram_length(), 4);
        assert_eq!(c.ngrams[&4].len(), 16);
        // Lookup via CorpusSource trait now sees derived data.
        let p = c.ngram_freq(&['a', 'a', 'a', 'a']);
        assert!((p - (0.2 * 0.2 / 0.3)).abs() < 1e-12);
    }

    #[test]
    fn ensure_ngrams_is_idempotent() {
        let mut c = tiny_corpus();
        c.ensure_ngrams(4).unwrap();
        let first_len = c.ngrams[&4].len();
        c.ensure_ngrams(4).unwrap(); // second call
        assert_eq!(c.ngrams[&4].len(), first_len);
    }

    #[test]
    fn ensure_ngrams_chains_to_5_with_correct_math() {
        let mut c = tiny_corpus();
        c.ensure_ngrams(5).unwrap();
        assert!(c.ngrams.contains_key(&4), "level 4 derived as prereq");
        assert!(c.ngrams.contains_key(&5), "level 5 derived from level 4");
        assert_eq!(c.max_ngram_length(), 5);

        // Hand-computed check that chaining produced real numbers,
        // not garbage (idempotent caching of a broken chain would
        // still satisfy the shape-only assertions above).
        //
        //   P(aaaa) = P(aaa) × P(aaa) / P(aa) = 0.2 × 0.2 / 0.3
        //   P(aaaaa) = P(aaaa) × P(aaaa) / P(aaa)
        let p4_aaaa = 0.2 * 0.2 / 0.3;
        let expect = p4_aaaa * p4_aaaa / 0.2;
        let got = c.ngram_freq(&['a', 'a', 'a', 'a', 'a']);
        assert!(
            (got - expect).abs() < 1e-12,
            "chained 5-gram math: got {got}, want {expect}"
        );
    }

    #[test]
    fn ensure_ngrams_noop_for_small_n() {
        let mut c = tiny_corpus();
        c.ensure_ngrams(3).unwrap();
        c.ensure_ngrams(2).unwrap();
        c.ensure_ngrams(1).unwrap();
        assert!(c.ngrams.is_empty(), "nothing to derive for n <= 3");
    }
}
