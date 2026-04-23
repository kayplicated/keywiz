//! Flexion-cascade analyzer.
//!
//! Rewards same-hand trigrams confined to a configurable row set
//! (default `["home", "bottom"]`) with at least one row transition.
//! The row set is a config parameter, so an extension-philosophy
//! user can flip it to `["home", "top"]` — but in practice the
//! mirror analyzer [`crate::extension_cascade`] covers that case
//! with a clearer name.

use anyhow::Result;
use drift_analyzer::{f64_or, strings_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Hit, Row, Scope, Window};

use crate::row_util::parse_row_name;

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "flexion_cascade",
        build: |cfg| Ok(Box::new(FlexionCascade::from_config(cfg)?)),
    });
}

pub struct FlexionCascade {
    pub weight: f64,
    pub allowed_rows: Vec<Row>,
}

impl FlexionCascade {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        let row_names = strings_or(cfg, "allowed_rows", &["home", "bottom"]);
        let allowed_rows: Vec<Row> = row_names
            .iter()
            .filter_map(|n| parse_row_name(n))
            .collect();
        Ok(Self {
            weight: f64_or(cfg, "weight", 1.5),
            allowed_rows,
        })
    }
}

impl Analyzer for FlexionCascade {
    fn name(&self) -> &'static str {
        "flexion_cascade"
    }

    fn scope(&self) -> Scope {
        Scope::Trigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let p = window.props;
        if !p.all_same_hand {
            return Vec::new();
        }
        for row in &p.rows {
            if !self.allowed_rows.contains(row) {
                return Vec::new();
            }
        }
        // Must have at least one row transition; a same-row sequence
        // is already handled by roll/onehand.
        let varies = p.rows[0] != p.rows[1] || p.rows[1] != p.rows[2];
        if !varies {
            return Vec::new();
        }

        let [a, b, c] = [window.chars[0], window.chars[1], window.chars[2]];
        vec![Hit {
            category: "flexion_cascade",
            label: format!("{a}{b}{c}"),
            cost: window.freq * self.weight,
        }]
    }
}
