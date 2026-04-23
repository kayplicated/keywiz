//! Stretch analyzer — non-adjacent same-hand cross-row bigram.
//!
//! Covers pinky-to-middle type motions where a scissor analyzer
//! wouldn't match (finger gap > 1). Small flat penalty rather than
//! a direction-aware one — stretches across multiple columns don't
//! have a neat flexion/extension asymmetry.

use anyhow::Result;
use drift_analyzer::{f64_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Hit, Scope, Window};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "stretch",
        build: |cfg| Ok(Box::new(Stretch::from_config(cfg)?)),
    });
}

pub struct Stretch {
    pub penalty: f64,
}

impl Stretch {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        Ok(Self {
            penalty: f64_or(cfg, "penalty", -1.0),
        })
    }
}

impl Analyzer for Stretch {
    fn name(&self) -> &'static str {
        "stretch"
    }

    fn scope(&self) -> Scope {
        Scope::Bigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let a = window.keys[0];
        let b = window.keys[1];

        if !a.finger.same_hand(b.finger) || a.finger == b.finger {
            return Vec::new();
        }
        let Some(gap) = a.finger.column_distance(b.finger) else {
            return Vec::new();
        };
        // Non-adjacent and cross-row: stretch.
        if gap <= 1 || a.row == b.row {
            return Vec::new();
        }

        vec![Hit {
            category: "stretch",
            label: format!("Stretch {}{}", window.chars[0], window.chars[1]),
            cost: window.freq * self.penalty,
        }]
    }
}
