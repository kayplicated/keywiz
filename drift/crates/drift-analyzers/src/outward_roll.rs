//! Trigram outward-roll analyzer.
//!
//! Mirrors [`InwardRoll`](crate::inward_roll::InwardRoll) for
//! monotonic-outward trigrams (index → middle → ring → pinky).

use anyhow::Result;
use drift_analyzer::{f64_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Finger, Hit, Scope, Window};

use crate::trigram_util::{is_roll3_outward, is_roll3_outward_skip};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "outward_roll",
        build: |cfg| Ok(Box::new(OutwardRoll::from_config(cfg)?)),
    });
}

pub struct OutwardRoll {
    pub weight: f64,
    pub skip_multiplier: f64,
    pub end_on_pinky_multiplier: f64,
}

impl OutwardRoll {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        Ok(Self {
            weight: f64_or(cfg, "weight", 2.5),
            skip_multiplier: f64_or(cfg, "skip_multiplier", 0.7),
            end_on_pinky_multiplier: f64_or(cfg, "end_on_pinky_multiplier", 1.0),
        })
    }
}

impl Analyzer for OutwardRoll {
    fn name(&self) -> &'static str {
        "outward_roll"
    }

    fn scope(&self) -> Scope {
        Scope::Trigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let kind_mult = if is_roll3_outward(window.props) {
            1.0
        } else if is_roll3_outward_skip(window.props) {
            self.skip_multiplier
        } else {
            return Vec::new();
        };

        let terminal_mult = match window.keys[2].finger {
            Finger::LPinky | Finger::RPinky => self.end_on_pinky_multiplier,
            _ => 1.0,
        };

        let [a, b, c] = [window.chars[0], window.chars[1], window.chars[2]];
        vec![Hit {
            category: "outward_roll",
            label: format!("{a}{b}{c}"),
            cost: window.freq * self.weight * kind_mult * terminal_mult,
        }]
    }
}
