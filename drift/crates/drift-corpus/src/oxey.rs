//! Oxey JSON corpus format reader.
//!
//! Reads the oxeylyzer corpus schema. All frequencies are
//! percentages (already normalized to 100). The oxey format has
//! grown over time:
//!
//! - `chars` and `bigrams` are mandatory.
//! - `trigrams` is common but optional.
//! - `skipgrams`, `skipgrams2`, `skipgrams3` are char pairs with
//!   1/2/3 characters skipped between them. Optional.
//!
//! Anything missing deserializes as an empty map.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::MemoryCorpus;

#[derive(Debug, Deserialize)]
struct RawCorpus {
    name: String,
    chars: HashMap<String, f64>,
    bigrams: HashMap<String, f64>,
    #[serde(default)]
    trigrams: HashMap<String, f64>,
    #[serde(default)]
    skipgrams: HashMap<String, f64>,
    #[serde(default)]
    skipgrams2: HashMap<String, f64>,
    #[serde(default)]
    skipgrams3: HashMap<String, f64>,
}

/// Load an oxey JSON file into a [`MemoryCorpus`].
pub fn load(path: &Path) -> Result<MemoryCorpus> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading corpus: {}", path.display()))?;
    let raw: RawCorpus = serde_json::from_str(&text)
        .with_context(|| format!("parsing corpus: {}", path.display()))?;

    let chars = raw
        .chars
        .into_iter()
        .filter_map(|(k, v)| {
            let ch = k.chars().next()?;
            Some((ch, v))
        })
        .collect();

    let bigrams = parse_pair_map(raw.bigrams);
    let trigrams = raw
        .trigrams
        .into_iter()
        .filter_map(|(k, v)| {
            let mut it = k.chars();
            let a = it.next()?;
            let b = it.next()?;
            let c = it.next()?;
            if it.next().is_some() {
                return None;
            }
            Some(((a, b, c), v))
        })
        .collect();

    // Stash each skipgram table under its gap.
    let mut skipgrams: HashMap<usize, HashMap<(char, char), f64>> = HashMap::new();
    let sg1 = parse_pair_map(raw.skipgrams);
    if !sg1.is_empty() {
        skipgrams.insert(1, sg1);
    }
    let sg2 = parse_pair_map(raw.skipgrams2);
    if !sg2.is_empty() {
        skipgrams.insert(2, sg2);
    }
    let sg3 = parse_pair_map(raw.skipgrams3);
    if !sg3.is_empty() {
        skipgrams.insert(3, sg3);
    }

    Ok(MemoryCorpus {
        name: raw.name,
        chars,
        bigrams,
        trigrams,
        ngrams: HashMap::new(),
        skipgrams,
    })
}

/// Parse a `String -> f64` map whose keys are 2-character strings
/// into a `(char, char) -> f64` map. Longer or shorter keys are
/// dropped.
fn parse_pair_map(raw: HashMap<String, f64>) -> HashMap<(char, char), f64> {
    raw.into_iter()
        .filter_map(|(k, v)| {
            let mut it = k.chars();
            let a = it.next()?;
            let b = it.next()?;
            if it.next().is_some() {
                return None;
            }
            Some(((a, b), v))
        })
        .collect()
}
