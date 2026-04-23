//! Per-finger trigram terminal penalty.
//!
//! Generalization of the older `pinky_terminal` rule. Each finger
//! carries its own penalty weight (default 0). When a same-hand
//! trigram lands on finger X, weight[X] is added as the hit cost
//! (typically negative).
//!
//! Users who want only the pinky singled out set pinky weights
//! negative and leave the rest at zero; users with a different
//! unstable finger adjust as they prefer. No finger is hardcoded.

use std::collections::HashMap;

use anyhow::Result;
use drift_analyzer::{f64_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Finger, Hit, Scope, Window};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "terminal_penalty",
        build: |cfg| Ok(Box::new(TerminalPenalty::from_config(cfg)?)),
    });
}

pub struct TerminalPenalty {
    pub per_finger: HashMap<Finger, f64>,
}

impl TerminalPenalty {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        let mut per_finger = HashMap::new();
        per_finger.insert(Finger::LPinky, f64_or(cfg, "l_pinky", 0.0));
        per_finger.insert(Finger::LRing, f64_or(cfg, "l_ring", 0.0));
        per_finger.insert(Finger::LMiddle, f64_or(cfg, "l_middle", 0.0));
        per_finger.insert(Finger::LIndex, f64_or(cfg, "l_index", 0.0));
        per_finger.insert(Finger::RIndex, f64_or(cfg, "r_index", 0.0));
        per_finger.insert(Finger::RMiddle, f64_or(cfg, "r_middle", 0.0));
        per_finger.insert(Finger::RRing, f64_or(cfg, "r_ring", 0.0));
        per_finger.insert(Finger::RPinky, f64_or(cfg, "r_pinky", 0.0));
        Ok(Self { per_finger })
    }
}

impl Analyzer for TerminalPenalty {
    fn name(&self) -> &'static str {
        "terminal_penalty"
    }

    fn scope(&self) -> Scope {
        Scope::Trigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        if !window.props.all_same_hand {
            return Vec::new();
        }
        let terminal = window.keys[2].finger;
        let weight = self.per_finger.get(&terminal).copied().unwrap_or(0.0);
        if weight == 0.0 {
            return Vec::new();
        }
        let [a, b, c] = [window.chars[0], window.chars[1], window.chars[2]];
        vec![Hit {
            category: "terminal_penalty",
            label: format!("{a}{b}{c}"),
            cost: window.freq * weight,
        }]
    }
}
