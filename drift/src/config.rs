//! Configuration loader for `drift.toml`.
//!
//! Holds every tunable weight used by the scorer. Anything that
//! shapes the numerical output lives here so it can be tweaked
//! without recompiling.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Root config struct. Mirrors `drift.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub corpus: CorpusConfig,
    pub bigram: BigramWeights,
    pub roll: RollWeights,
    pub row: RowWeights,
    pub finger: FingerWeights,
    pub asymmetric: AsymmetricRules,
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
        let cfg: Config = toml::from_str(&text)
            .with_context(|| format!("parsing config: {}", path.display()))?;
        Ok(cfg)
    }

    /// Load `drift.toml` from the crate root.
    pub fn load_default() -> Result<Self> {
        let crate_root = env!("CARGO_MANIFEST_DIR");
        let path = Path::new(crate_root).join("drift.toml");
        Self::load_from(&path)
    }
}
