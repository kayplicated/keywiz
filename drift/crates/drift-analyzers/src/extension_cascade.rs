//! Extension-cascade analyzer.
//!
//! Mirror of [`crate::flexion_cascade`]: rewards same-hand trigrams
//! confined to a row set with at least one row transition, but
//! defaults to `["home", "top"]` so users biased toward extension-
//! preferring layouts can earn the analogous bonus without needing
//! a different rule module.
//!
//! The analyzer is structurally identical to flexion_cascade and
//! could be collapsed into a single generic "row_cascade_reward"
//! rule. Keeping them separate so each philosophy has a rule with
//! a name that matches its intent.

use anyhow::Result;
use drift_analyzer::{f64_or, strings_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Hit, Row, Scope, Window};

use crate::row_util::parse_row_name;

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "extension_cascade",
        build: |cfg| Ok(Box::new(ExtensionCascade::from_config(cfg)?)),
    });
}

pub struct ExtensionCascade {
    pub weight: f64,
    pub allowed_rows: Vec<Row>,
}

impl ExtensionCascade {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        let row_names = strings_or(cfg, "allowed_rows", &["home", "top"]);
        let allowed_rows: Vec<Row> = row_names
            .iter()
            .filter_map(|n| parse_row_name(n))
            .collect();
        Ok(Self {
            weight: f64_or(cfg, "weight", 0.0),
            allowed_rows,
        })
    }
}

impl Analyzer for ExtensionCascade {
    fn name(&self) -> &'static str {
        "extension_cascade"
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
        let varies = p.rows[0] != p.rows[1] || p.rows[1] != p.rows[2];
        if !varies {
            return Vec::new();
        }

        let [a, b, c] = [window.chars[0], window.chars[1], window.chars[2]];
        vec![Hit {
            category: "extension_cascade",
            label: format!("{a}{b}{c}"),
            cost: window.freq * self.weight,
        }]
    }
}
