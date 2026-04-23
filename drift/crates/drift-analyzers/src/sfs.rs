//! Same-finger skipgram analyzer.
//!
//! A **SFS** (same-finger skipgram) is a pair of characters typed
//! by the same finger with `gap` other characters between them in
//! the source text. Unlike a pure SFB the finger has an intervening
//! beat on the *other* hand (usually) to recover, so the penalty
//! is smaller — but it's still the same finger lifting and
//! returning within a short window.
//!
//! Like [`crate::sfb`], this analyzer distinguishes vertical
//! (same sub-column) from lateral (index-column-crossing) motions.
//! The lateral case is even milder here than in the SFB case
//! because the recovery beat makes the column shift essentially
//! free.

use anyhow::Result;
use drift_analyzer::{f64_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Hit, Scope, Window};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "sfs",
        build: |cfg| Ok(Box::new(Sfs::from_config(cfg)?)),
    });
}

pub struct Sfs {
    pub penalty: f64,
    pub lateral_penalty: f64,
    pub gap: usize,
}

impl Sfs {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        let gap = cfg
            .and_then(|c| c.get("gap"))
            .and_then(|v| v.as_i64())
            .map(|i| i.max(1) as usize)
            .unwrap_or(1);
        Ok(Self {
            penalty: f64_or(cfg, "penalty", -2.0),
            lateral_penalty: f64_or(cfg, "lateral_penalty", -0.5),
            gap,
        })
    }
}

impl Analyzer for Sfs {
    fn name(&self) -> &'static str {
        "sfs"
    }

    fn scope(&self) -> Scope {
        Scope::Skipgram(self.gap)
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let a = window.keys[0];
        let b = window.keys[1];
        if a.finger != b.finger || a.id == b.id {
            return Vec::new();
        }
        let (prefix, cost) = if a.same_finger_column(b) {
            ("SFS", window.freq * self.penalty)
        } else {
            ("SFS-lat", window.freq * self.lateral_penalty)
        };
        vec![Hit {
            category: "sfs",
            label: format!(
                "{}({}) {}{}",
                prefix, self.gap, window.chars[0], window.chars[1]
            ),
            cost,
        }]
    }
}
