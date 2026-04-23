//! Bigram roll analyzer — same-row, adjacent fingers.
//!
//! Rewards same-row adjacent-finger bigrams with a direction-aware
//! multiplier. The trigram-level inward/outward roll analyzers live
//! in separate files.

use anyhow::Result;
use drift_analyzer::{f64_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Hit, Scope, Window};
use drift_motion::{roll_direction, RollDirection};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "roll",
        build: |cfg| Ok(Box::new(Roll::from_config(cfg)?)),
    });
}

pub struct Roll {
    pub same_row_adjacent: f64,
    pub inward_multiplier: f64,
    pub outward_multiplier: f64,
}

impl Roll {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        Ok(Self {
            same_row_adjacent: f64_or(cfg, "same_row_adjacent", 2.0),
            inward_multiplier: f64_or(cfg, "inward_multiplier", 1.0),
            outward_multiplier: f64_or(cfg, "outward_multiplier", 1.0),
        })
    }
}

impl Analyzer for Roll {
    fn name(&self) -> &'static str {
        "roll"
    }

    fn scope(&self) -> Scope {
        Scope::Bigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let a = window.keys[0];
        let b = window.keys[1];

        // Same hand, adjacent fingers, same row.
        if !a.finger.same_hand(b.finger)
            || a.finger == b.finger
            || a.row != b.row
            || a.finger.column_distance(b.finger) != Some(1)
        {
            return Vec::new();
        }

        let (label_dir, mult) = match roll_direction(a.finger, b.finger) {
            Some(RollDirection::Inward) => ("Inward", self.inward_multiplier),
            Some(RollDirection::Outward) => ("Outward", self.outward_multiplier),
            None => return Vec::new(),
        };

        vec![Hit {
            category: "roll",
            label: format!("{} {}{}", label_dir, window.chars[0], window.chars[1]),
            cost: window.freq * self.same_row_adjacent * mult,
        }]
    }
}
