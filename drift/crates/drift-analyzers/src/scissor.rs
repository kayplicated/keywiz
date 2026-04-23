//! Scissor analyzer — adjacent-finger cross-row bigram.
//!
//! Applies a direction-aware penalty to adjacent-finger bigrams
//! that cross rows, after consulting drift-motion's
//! asymmetric-forward exemption rule. The three per-pair toggles
//! (`index_middle_forward_ok`, `middle_ring_forward_ok`,
//! `ring_pinky_forward_ok`) are read here because the exemption is
//! specific to what the analyzer considers a scissor.

use anyhow::Result;
use drift_analyzer::{bool_or, f64_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Hit, Scope, Window};
use drift_motion::{cross_row_kind, is_forward_exempt, AsymmetricRules, CrossRowKind};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "scissor",
        build: |cfg| Ok(Box::new(Scissor::from_config(cfg)?)),
    });
}

pub struct Scissor {
    pub base_penalty: f64,
    pub flexion: f64,
    pub extension: f64,
    pub full_cross: f64,
    pub asymmetric: AsymmetricRules,
}

impl Scissor {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        Ok(Self {
            base_penalty: f64_or(cfg, "base_penalty", -2.0),
            flexion: f64_or(cfg, "flexion", 1.0),
            extension: f64_or(cfg, "extension", 1.0),
            full_cross: f64_or(cfg, "full_cross", 1.2),
            asymmetric: AsymmetricRules {
                index_middle_forward_ok: bool_or(cfg, "index_middle_forward_ok", true),
                middle_ring_forward_ok: bool_or(cfg, "middle_ring_forward_ok", true),
                ring_pinky_forward_ok: bool_or(cfg, "ring_pinky_forward_ok", true),
                forward_threshold: f64_or(cfg, "forward_threshold", 0.0),
            },
        })
    }
}

impl Analyzer for Scissor {
    fn name(&self) -> &'static str {
        "scissor"
    }

    fn scope(&self) -> Scope {
        Scope::Bigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let a = window.keys[0];
        let b = window.keys[1];

        if !a.finger.same_hand(b.finger)
            || a.finger == b.finger
            || a.finger.column_distance(b.finger) != Some(1)
            || a.row == b.row
        {
            return Vec::new();
        }

        // Asymmetric-forward exemption — natural hand splay on
        // col-stag boards isn't a scissor.
        if is_forward_exempt(a, b, &self.asymmetric) {
            return Vec::new();
        }

        let (label, mult) = match cross_row_kind(a.row, b.row) {
            CrossRowKind::Flexion => ("Flexion", self.flexion),
            CrossRowKind::Extension => ("Extension", self.extension),
            CrossRowKind::FullCross => ("FullCross", self.full_cross),
            CrossRowKind::Other => return Vec::new(),
        };

        vec![Hit {
            category: "scissor",
            label: format!("{} {}{}", label, window.chars[0], window.chars[1]),
            cost: window.freq * self.base_penalty * mult,
        }]
    }
}
