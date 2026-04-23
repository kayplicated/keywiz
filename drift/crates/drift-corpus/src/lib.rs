//! Corpus loaders and in-memory implementations of
//! [`CorpusSource`](drift_core::CorpusSource).
//!
//! Reads the oxey-compatible JSON format. Supports blending several
//! corpora with weights. Higher-order n-grams may be derived on
//! demand when the source corpus doesn't supply them.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Result};
use drift_core::CorpusSource;

pub mod derive;
pub mod memory;
pub mod oxey;

pub use memory::MemoryCorpus;

/// Load an oxey-format JSON corpus into memory.
pub fn load(path: &Path) -> Result<MemoryCorpus> {
    oxey::load(path)
}

/// Weighted blend of multiple corpora. Input weights are
/// normalized; the result is a single `MemoryCorpus` with
/// per-entry frequencies linearly combined.
///
/// The inputs are dyn sources rather than a concrete type so any
/// [`CorpusSource`] implementation can participate in a blend,
/// including future streaming or memory-mapped sources.
pub fn blend(inputs: &[(Box<dyn CorpusSource>, f64)]) -> Result<MemoryCorpus> {
    if inputs.is_empty() {
        bail!("blend requires at least one corpus");
    }
    let total: f64 = inputs.iter().map(|(_, w)| w).sum();
    if total <= 0.0 {
        bail!("blend weights sum to 0");
    }

    let mut chars: HashMap<char, f64> = HashMap::new();
    let mut bigrams: HashMap<(char, char), f64> = HashMap::new();
    let mut trigrams: HashMap<(char, char, char), f64> = HashMap::new();
    // Blend skipgrams per-gap. We probe the common three gaps since
    // that's what the oxey format stores; higher gaps just won't
    // contribute unless a source knows about them.
    let mut skipgrams: HashMap<usize, HashMap<(char, char), f64>> = HashMap::new();

    for (source, weight) in inputs {
        let share = weight / total;
        for (ch, freq) in source.iter_chars() {
            *chars.entry(ch).or_insert(0.0) += freq * share;
        }
        for (pair, freq) in source.iter_bigrams() {
            *bigrams.entry(pair).or_insert(0.0) += freq * share;
        }
        for (tri, freq) in source.iter_trigrams() {
            *trigrams.entry(tri).or_insert(0.0) += freq * share;
        }
        for gap in [1, 2, 3] {
            for (pair, freq) in source.iter_skipgrams(gap) {
                *skipgrams
                    .entry(gap)
                    .or_default()
                    .entry(pair)
                    .or_insert(0.0) += freq * share;
            }
        }
    }

    // Drop any gap whose map is empty (no source supplied data).
    skipgrams.retain(|_, m| !m.is_empty());

    let name = inputs
        .iter()
        .map(|(c, w)| format!("{}:{:.2}", c.name(), w / total))
        .collect::<Vec<_>>()
        .join("+");

    Ok(MemoryCorpus {
        name: format!("blend[{name}]"),
        chars,
        bigrams,
        trigrams,
        ngrams: HashMap::new(),
        skipgrams,
    })
}
