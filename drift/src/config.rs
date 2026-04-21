//! Configuration loader for `drift.toml`.
//!
//! Holds every tunable weight used by the scorer. Anything that
//! shapes the numerical output lives here so it can be tweaked
//! without recompiling.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use toml::Value;

/// Root config struct. Mirrors `drift.toml`. Sections relevant to
/// the bigram pipeline deserialize directly; the `[trigram]` section
/// is kept as a raw [`Value`] so its pluggable-rule subtables can
/// be parsed by the trigram registry at pipeline-build time.
#[derive(Debug, Clone)]
pub struct Config {
    pub corpus: CorpusConfig,
    pub bigram: BigramWeights,
    pub roll: RollWeights,
    pub row: RowWeights,
    pub finger: FingerWeights,
    pub asymmetric: AsymmetricRules,
    /// Raw `[trigram]` subtable; `None` if the section is absent.
    pub trigram: Option<Value>,
}

/// Intermediate struct used purely for deserialization. All
/// strongly-typed fields go through this; `[trigram]` is captured
/// separately to preserve its pluggable shape.
#[derive(Debug, Clone, Deserialize)]
struct RawConfig {
    corpus: CorpusConfig,
    bigram: BigramWeights,
    roll: RollWeights,
    row: RowWeights,
    finger: FingerWeights,
    asymmetric: AsymmetricRules,
}

/// Corpus-related settings.
#[derive(Debug, Clone, Deserialize)]
pub struct CorpusConfig {
    /// Default corpus path, used when `--corpus` is not passed.
    /// Interpreted relative to the crate root.
    pub default: PathBuf,
}

/// Weights applied to bigram motion types.
#[derive(Debug, Clone, Deserialize)]
pub struct BigramWeights {
    pub sfb_penalty: f64,
    pub scissor_penalty: f64,
    pub lateral_penalty: f64,
}

/// Reward weights for clean roll patterns.
#[derive(Debug, Clone, Deserialize)]
pub struct RollWeights {
    pub same_row_adjacent: f64,
    pub inward_multiplier: f64,
    pub outward_multiplier: f64,
}

/// Row-direction cost multipliers. Applied on top of `scissor_penalty`.
#[derive(Debug, Clone, Deserialize)]
pub struct RowWeights {
    pub flexion: f64,
    pub extension: f64,
    pub full_cross: f64,
}

/// Per-finger load weights. Higher weight = finger can take more work
/// cheaply.
#[derive(Debug, Clone, Deserialize)]
pub struct FingerWeights {
    /// Multiplier on the quadratic overload penalty.
    /// Set to 0 to disable finger-load's contribution to total score.
    pub overload_penalty: f64,
    pub left_pinky: f64,
    pub left_ring: f64,
    pub left_middle: f64,
    pub left_index: f64,
    pub right_index: f64,
    pub right_middle: f64,
    pub right_ring: f64,
    pub right_pinky: f64,
}

/// Which finger pairs get the "outer-finger-forward-is-natural" rule.
#[derive(Debug, Clone, Deserialize)]
pub struct AsymmetricRules {
    pub index_middle_forward_ok: bool,
    pub middle_ring_forward_ok: bool,
    pub ring_pinky_forward_ok: bool,
}

impl Config {
    /// Load from an explicit path. Use [`load_default`] for the
    /// standard `drift.toml` lookup.
    pub fn load_from(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config: {}", path.display()))?;
        let raw: RawConfig = toml::from_str(&text)
            .with_context(|| format!("parsing config: {}", path.display()))?;
        let value: Value = toml::from_str(&text)
            .with_context(|| format!("reparsing config as Value: {}", path.display()))?;
        let trigram = value
            .as_table()
            .and_then(|t| t.get("trigram"))
            .cloned();
        Ok(Config {
            corpus: raw.corpus,
            bigram: raw.bigram,
            roll: raw.roll,
            row: raw.row,
            finger: raw.finger,
            asymmetric: raw.asymmetric,
            trigram,
        })
    }

    /// Load `drift.toml` from the crate root.
    pub fn load_default() -> Result<Self> {
        let crate_root = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(crate_root).join("drift.toml");
        Self::load_from(&path)
    }
}
