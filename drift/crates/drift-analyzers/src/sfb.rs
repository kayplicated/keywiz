//! Same-finger bigram analyzer.
//!
//! Fires when both keys in a bigram are typed by the same finger.
//! Splits the cost into two cases:
//!
//! - **Vertical SFB** — both keys are in the same finger
//!   sub-column. The finger lifts off one key, travels vertically,
//!   and lands on another. This is the painful textbook SFB. Uses
//!   the `penalty` weight.
//! - **Lateral same-finger** — both keys are on the same finger
//!   but different sub-columns (only possible for index fingers on
//!   boards where one finger owns two columns: a primary column
//!   and an inner column reached across the board's central gap).
//!   The finger shifts horizontally without a full vertical
//!   return. Still effortful — a roll would be cheaper — but
//!   significantly less painful than a vertical SFB. Uses the
//!   `lateral_penalty` weight.
//!
//! Setting `lateral_penalty` to the same value as `penalty`
//! reproduces the pre-refactor behavior where both cases were
//! penalized equally.

use anyhow::Result;
use drift_analyzer::{f64_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Hit, Scope, Window};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "sfb",
        build: |cfg| Ok(Box::new(Sfb::from_config(cfg)?)),
    });
}

pub struct Sfb {
    pub penalty: f64,
    pub lateral_penalty: f64,
}

impl Sfb {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        Ok(Self {
            penalty: f64_or(cfg, "penalty", -7.0),
            lateral_penalty: f64_or(cfg, "lateral_penalty", -2.0),
        })
    }
}

impl Analyzer for Sfb {
    fn name(&self) -> &'static str {
        "sfb"
    }

    fn scope(&self) -> Scope {
        Scope::Bigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let a = window.keys[0];
        let b = window.keys[1];
        if a.finger != b.finger || a.id == b.id {
            return Vec::new();
        }
        // Same finger. Classify by sub-column.
        let (label, cost) = if a.same_finger_column(b) {
            (
                format!("SFB {}{}", window.chars[0], window.chars[1]),
                window.freq * self.penalty,
            )
        } else {
            (
                format!("SFB-lat {}{}", window.chars[0], window.chars[1]),
                window.freq * self.lateral_penalty,
            )
        };
        vec![Hit {
            category: "sfb",
            label,
            cost,
        }]
    }
}
