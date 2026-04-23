//! Async hand-drift analyzer.
//!
//! Penalizes n-gram windows where the two hands drift in opposite
//! vertical directions — one hand visiting rows above home while
//! the other visits rows below home. Captures sustained row-split
//! patterns that complement [`crate::hand_territory`]'s per-pair
//! row-delta scoring.
//!
//! The rule fires on trigrams (and higher-n windows, if ngram-scope
//! corpus data is available) where:
//!
//! - at least one pair in the window is cross-hand (otherwise it's
//!   a same-hand pattern that other rules own)
//! - both hands have characters in the window (i.e. the window
//!   isn't entirely on one hand)
//! - the two hands' mean-row offsets have strictly opposite signs,
//!   where Home is zero, Top is negative, Bottom is positive
//!
//! Cost scales with the product of the two hands' absolute mean
//! offsets, times frequency, times the configured weight. A
//! window with `L=Home,Top` and `R=Home,Bottom` scores heavier
//! than one with just `L=Top` and `R=Bottom` on single keys.

use anyhow::{Result, bail};
use drift_analyzer::{Analyzer, AnalyzerEntry, ConfigValue, Registry, f64_or};
use drift_core::{Hit, Scope, Window};

use crate::row_util::row_index;

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "async_hand_drift",
        build: |cfg| Ok(Box::new(AsyncHandDrift::from_config(cfg)?)),
    });
}

pub struct AsyncHandDrift {
    /// Penalty multiplier applied to freq × span_product. Default
    /// 0 means the analyzer is inert unless a preset opts in.
    pub weight: f64,
    /// Window length. 3 = trigrams (default, cheapest). 4+ asks the
    /// corpus to supply derived n-gram data; see
    /// [`drift_corpus::MemoryCorpus::ensure_ngrams`]. Presets that
    /// want longer-horizon drift detection opt in via the config.
    pub length: usize,
}

impl AsyncHandDrift {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        let length = cfg
            .and_then(|c| c.get("length"))
            .and_then(|v| v.as_i64())
            .map(|v| v as usize)
            .unwrap_or(3);
        if length < 3 {
            bail!("async_hand_drift.length must be >= 3, got {length}");
        }
        Ok(Self {
            weight: f64_or(cfg, "weight", 0.0),
            length,
        })
    }
}

impl Analyzer for AsyncHandDrift {
    fn name(&self) -> &'static str {
        "async_hand_drift"
    }

    fn scope(&self) -> Scope {
        if self.length == 3 {
            Scope::Trigram
        } else {
            Scope::Ngram(self.length)
        }
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        if self.weight == 0.0 {
            return Vec::new();
        }
        let p = window.props;

        // Skip purely same-hand windows — those are the roll /
        // onehand / redirect domain.
        if p.all_same_hand {
            return Vec::new();
        }

        // Partition chars into left-hand and right-hand row offsets
        // relative to Home (0). Top rows contribute negative values,
        // Bottom rows positive.
        let mut left: Vec<i32> = Vec::new();
        let mut right: Vec<i32> = Vec::new();
        for (i, key) in window.keys.iter().enumerate() {
            let offset = row_index(p.rows[i]);
            if key.finger.hand() == drift_core::Hand::Left {
                left.push(offset);
            } else {
                right.push(offset);
            }
        }

        // Both hands must have at least one char; otherwise there's
        // no cross-hand drift to measure.
        if left.is_empty() || right.is_empty() {
            return Vec::new();
        }

        let l_mean = mean(&left);
        let r_mean = mean(&right);

        // Opposite non-zero signs only.
        if l_mean == 0.0 || r_mean == 0.0 || l_mean.signum() == r_mean.signum() {
            return Vec::new();
        }

        let span_product = l_mean.abs() * r_mean.abs();
        let label: String = window.chars.iter().collect();
        vec![Hit {
            category: "async_hand_drift",
            label,
            cost: window.freq * span_product * self.weight,
        }]
    }
}

fn mean(values: &[i32]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().map(|&v| v as f64).sum::<f64>() / values.len() as f64
}
