//! Row-cascade analyzer.
//!
//! Penalizes same-hand trigrams that visit all three alpha rows in
//! sequence (Top, Home, and Bottom all present). The hand has to
//! reshape on every keystroke — the "roller coaster" pattern.

use anyhow::Result;
use drift_analyzer::{f64_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Hit, Row, Scope, Window};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "row_cascade",
        build: |cfg| Ok(Box::new(RowCascade::from_config(cfg)?)),
    });
}

pub struct RowCascade {
    pub weight: f64,
}

impl RowCascade {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        Ok(Self {
            weight: f64_or(cfg, "weight", -3.0),
        })
    }
}

impl Analyzer for RowCascade {
    fn name(&self) -> &'static str {
        "row_cascade"
    }

    fn scope(&self) -> Scope {
        Scope::Trigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let p = window.props;
        if !p.all_same_hand {
            return Vec::new();
        }
        let mut has_top = false;
        let mut has_home = false;
        let mut has_bot = false;
        for row in &p.rows {
            match row {
                Row::Top => has_top = true,
                Row::Home => has_home = true,
                Row::Bottom => has_bot = true,
                _ => {}
            }
        }
        if !(has_top && has_home && has_bot) {
            return Vec::new();
        }

        let [a, b, c] = [window.chars[0], window.chars[1], window.chars[2]];
        vec![Hit {
            category: "row_cascade",
            label: format!("{a}{b}{c}"),
            cost: window.freq * self.weight,
        }]
    }
}
