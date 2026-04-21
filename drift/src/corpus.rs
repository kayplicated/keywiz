//! Corpus loader. Reads the oxey-shared JSON corpus format.
//!
//! Corpora store percentages (already normalized to 100). We keep
//! them as percentages throughout — all scoring works on per-mille
//! style fractions of total typing, not raw counts.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Parsed bigram-level corpus.
#[derive(Debug, Clone)]
pub struct Corpus {
    pub name: String,
    /// Source path, retained for reporting/debugging.
    #[allow(dead_code)]
    pub path: PathBuf,
    /// Per-char frequency (%).
    pub chars: HashMap<char, f64>,
    /// Per-bigram frequency (%). Key is the 2-char string unchanged.
    pub bigrams: HashMap<(char, char), f64>,
}

#[derive(Debug, Deserialize)]
struct RawCorpus {
    name: String,
    chars: HashMap<String, f64>,
    bigrams: HashMap<String, f64>,
}

impl Corpus {
    /// Load from an oxey-style corpus JSON.
    pub fn load(path: &Path) -> Result<Self> {
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

        let bigrams = raw
            .bigrams
            .into_iter()
            .filter_map(|(k, v)| {
                let mut it = k.chars();
                let a = it.next()?;
                let b = it.next()?;
                if it.next().is_some() {
                    // Skip longer strings if any slipped in.
                    return None;
                }
                Some(((a, b), v))
            })
            .collect();

        Ok(Corpus {
            name: raw.name,
            path: path.to_path_buf(),
            chars,
            bigrams,
        })
    }

    /// Look up bigram frequency. Returns 0 if the pair is absent.
    /// Currently only used by direct iteration; exposed for future
    /// lookup-heavy scoring paths.
    #[allow(dead_code)]
    pub fn bigram(&self, a: char, b: char) -> f64 {
        self.bigrams.get(&(a, b)).copied().unwrap_or(0.0)
    }

    /// Look up char frequency. Returns 0 if the char is absent.
    #[allow(dead_code)]
    pub fn char(&self, a: char) -> f64 {
        self.chars.get(&a).copied().unwrap_or(0.0)
    }

    /// Weighted average of multiple corpora.
    ///
    /// Input: `(corpus, weight)` pairs. Weights are normalized — you
    /// can pass `[(a, 2.0), (b, 1.0)]` and a will contribute 2/3 of
    /// the blend. All percentages are linearly combined.
    ///
    /// Name is constructed from the component names; path is cleared.
    pub fn blend(inputs: &[(Corpus, f64)]) -> Result<Corpus> {
        if inputs.is_empty() {
            anyhow::bail!("blend requires at least one corpus");
        }

        let total_weight: f64 = inputs.iter().map(|(_, w)| w).sum();
        if total_weight <= 0.0 {
            anyhow::bail!("blend weights sum to 0");
        }

        let mut chars: HashMap<char, f64> = HashMap::new();
        let mut bigrams: HashMap<(char, char), f64> = HashMap::new();

        for (corpus, weight) in inputs {
            let share = weight / total_weight;
            for (&ch, &freq) in &corpus.chars {
                *chars.entry(ch).or_insert(0.0) += freq * share;
            }
            for (&pair, &freq) in &corpus.bigrams {
                *bigrams.entry(pair).or_insert(0.0) += freq * share;
            }
        }

        let name = inputs
            .iter()
            .map(|(c, w)| format!("{}:{:.2}", c.name, w / total_weight))
            .collect::<Vec<_>>()
            .join("+");

        Ok(Corpus {
            name: format!("blend[{name}]"),
            path: PathBuf::new(),
            chars,
            bigrams,
        })
    }
}
