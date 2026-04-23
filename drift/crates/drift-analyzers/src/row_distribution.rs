//! Row-distribution analyzer.
//!
//! Aggregate-scope. Reads per-char load from the context, uses the
//! layout to resolve each char to its row, and emits one hit per
//! row describing the percentage of typing landing on that row.
//! Weights are typically zero (this is an informational analyzer)
//! but users can attach penalties if they want to actively discourage
//! e.g. bottom-row load.

use anyhow::Result;
use drift_analyzer::{f64_or, AggregateContext, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Hit, Row, Scope};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "row_distribution",
        build: |cfg| Ok(Box::new(RowDistribution::from_config(cfg)?)),
    });
}

pub struct RowDistribution {
    pub top_weight: f64,
    pub home_weight: f64,
    pub bottom_weight: f64,
}

impl RowDistribution {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        Ok(Self {
            top_weight: f64_or(cfg, "top_weight", 0.0),
            home_weight: f64_or(cfg, "home_weight", 0.0),
            bottom_weight: f64_or(cfg, "bottom_weight", 0.0),
        })
    }
}

impl Analyzer for RowDistribution {
    fn name(&self) -> &'static str {
        "row_distribution"
    }

    fn scope(&self) -> Scope {
        Scope::Aggregate
    }

    fn evaluate_aggregate(&self, ctx: &AggregateContext) -> Vec<Hit> {
        let mut top = 0.0;
        let mut home = 0.0;
        let mut bottom = 0.0;
        for (&ch, &load) in ctx.char_load {
            let Some(key) = ctx.layout.position(ch) else {
                continue;
            };
            match key.row {
                Row::Top => top += load,
                Row::Home => home += load,
                Row::Bottom => bottom += load,
                _ => {}
            }
        }

        vec![
            Hit {
                category: "row_distribution",
                label: format!("Top {top:.2}%"),
                cost: top * self.top_weight,
            },
            Hit {
                category: "row_distribution",
                label: format!("Home {home:.2}%"),
                cost: home * self.home_weight,
            },
            Hit {
                category: "row_distribution",
                label: format!("Bottom {bottom:.2}%"),
                cost: bottom * self.bottom_weight,
            },
        ]
    }
}
