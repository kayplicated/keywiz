//! Trigram inward-roll analyzer.
//!
//! Rewards same-hand trigrams whose fingers step monotonically
//! inward (pinky → ring → middle → index). Skip rolls (one step of
//! two fingers) score at a reduced fraction. Optionally dampens the
//! reward when the terminal finger is the pinky — some hands find
//! that landing unstable.

use anyhow::Result;
use drift_analyzer::{f64_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Finger, Hit, Scope, Window};

use crate::trigram_util::{is_roll3_inward, is_roll3_inward_skip};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "inward_roll",
        build: |cfg| Ok(Box::new(InwardRoll::from_config(cfg)?)),
    });
}

pub struct InwardRoll {
    pub weight: f64,
    pub skip_multiplier: f64,
    pub end_on_pinky_multiplier: f64,
}

impl InwardRoll {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        Ok(Self {
            weight: f64_or(cfg, "weight", 3.0),
            skip_multiplier: f64_or(cfg, "skip_multiplier", 0.7),
            end_on_pinky_multiplier: f64_or(cfg, "end_on_pinky_multiplier", 1.0),
        })
    }
}

impl Analyzer for InwardRoll {
    fn name(&self) -> &'static str {
        "inward_roll"
    }

    fn scope(&self) -> Scope {
        Scope::Trigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let kind_mult = if is_roll3_inward(window.props) {
            1.0
        } else if is_roll3_inward_skip(window.props) {
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
            category: "inward_roll",
            label: format!("{a}{b}{c}"),
            cost: window.freq * self.weight * kind_mult * terminal_mult,
        }]
    }
}
