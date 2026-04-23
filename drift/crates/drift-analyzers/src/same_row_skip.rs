//! Same-row-skip analyzer.
//!
//! Fires on bigrams where both keys are on the same hand, same row,
//! and different non-adjacent fingers. Examples: home-row pinky→
//! middle directly, skipping ring.
//!
//! Whether these motions count as rolls, penalties, or neutral
//! depends on the user. Default weight is zero (no effect). Positive
//! weights treat them as sweep-like rolls; negative weights penalize
//! them as mid-row jumps.

use anyhow::Result;
use drift_analyzer::{f64_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Hit, Scope, Window};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "same_row_skip",
        build: |cfg| Ok(Box::new(SameRowSkip::from_config(cfg)?)),
    });
}

pub struct SameRowSkip {
    pub weight: f64,
}

impl SameRowSkip {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        Ok(Self {
            weight: f64_or(cfg, "weight", 0.0),
        })
    }
}

impl Analyzer for SameRowSkip {
    fn name(&self) -> &'static str {
        "same_row_skip"
    }

    fn scope(&self) -> Scope {
        Scope::Bigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let a = window.keys[0];
        let b = window.keys[1];

        if !a.finger.same_hand(b.finger) || a.finger == b.finger || a.row != b.row {
            return Vec::new();
        }
        // Same hand, same row, different fingers — but not adjacent.
        if a.finger.column_distance(b.finger) == Some(1) {
            return Vec::new();
        }

        if self.weight == 0.0 {
            return Vec::new();
        }

        vec![Hit {
            category: "same_row_skip",
            label: format!("Skip {}{}", window.chars[0], window.chars[1]),
            cost: window.freq * self.weight,
        }]
    }
}
