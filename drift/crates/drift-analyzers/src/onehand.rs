//! Trigram onehand analyzer.
//!
//! Fires on same-hand trigrams that aren't clean rolls or
//! redirects. Represents sustained same-hand work with no clear
//! rhythmic structure.

use anyhow::Result;
use drift_analyzer::{f64_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Hit, Scope, Window};

use crate::trigram_util::{
    is_redirect, is_roll3_inward, is_roll3_inward_skip, is_roll3_outward, is_roll3_outward_skip,
};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "onehand",
        build: |cfg| Ok(Box::new(Onehand::from_config(cfg)?)),
    });
}

pub struct Onehand {
    pub weight: f64,
}

impl Onehand {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        Ok(Self {
            weight: f64_or(cfg, "weight", 1.0),
        })
    }
}

impl Analyzer for Onehand {
    fn name(&self) -> &'static str {
        "onehand"
    }

    fn scope(&self) -> Scope {
        Scope::Trigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let p = window.props;
        if !p.all_same_hand {
            return Vec::new();
        }
        if is_roll3_inward(p)
            || is_roll3_outward(p)
            || is_roll3_inward_skip(p)
            || is_roll3_outward_skip(p)
            || is_redirect(p)
        {
            return Vec::new();
        }
        let [a, b, c] = [window.chars[0], window.chars[1], window.chars[2]];
        vec![Hit {
            category: "onehand",
            label: format!("{a}{b}{c}"),
            cost: window.freq * self.weight,
        }]
    }
}
