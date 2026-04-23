//! Finger-load analyzer — quadratic overload penalty.
//!
//! Aggregate-scope. Each finger has a configured "strength" (a
//! relative weight). The finger's fair share of total load is
//! `total * strength / sum_of_strengths`. Load beyond that share
//! incurs a squared penalty, so one badly-overloaded finger hurts
//! much more than a balanced distribution.
//!
//! Defaults: all strengths 1.0 (neutral, balance-only), overload
//! weight 0.0 (rule is disabled by default — opt-in). The
//! Drifter-philosophy preset ships non-neutral strengths; the
//! neutral preset leaves them at 1.0.

use std::collections::HashMap;

use anyhow::Result;
use drift_analyzer::{f64_or, AggregateContext, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Finger, Hit, Scope};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "finger_load",
        build: |cfg| Ok(Box::new(FingerLoad::from_config(cfg)?)),
    });
}

pub struct FingerLoad {
    /// Multiplier applied to the summed squared-excess. Typically
    /// negative (overload is a penalty). Default 0 disables the
    /// rule — no contribution to total score.
    pub overload_weight: f64,
    pub strengths: HashMap<Finger, f64>,
}

impl FingerLoad {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        let mut strengths = HashMap::new();
        strengths.insert(Finger::LPinky, f64_or(cfg, "l_pinky", 1.0));
        strengths.insert(Finger::LRing, f64_or(cfg, "l_ring", 1.0));
        strengths.insert(Finger::LMiddle, f64_or(cfg, "l_middle", 1.0));
        strengths.insert(Finger::LIndex, f64_or(cfg, "l_index", 1.0));
        strengths.insert(Finger::RIndex, f64_or(cfg, "r_index", 1.0));
        strengths.insert(Finger::RMiddle, f64_or(cfg, "r_middle", 1.0));
        strengths.insert(Finger::RRing, f64_or(cfg, "r_ring", 1.0));
        strengths.insert(Finger::RPinky, f64_or(cfg, "r_pinky", 1.0));
        Ok(Self {
            overload_weight: f64_or(cfg, "overload_weight", 0.0),
            strengths,
        })
    }
}

impl Analyzer for FingerLoad {
    fn name(&self) -> &'static str {
        "finger_load"
    }

    fn scope(&self) -> Scope {
        Scope::Aggregate
    }

    fn evaluate_aggregate(&self, ctx: &AggregateContext) -> Vec<Hit> {
        if self.overload_weight == 0.0 {
            return Vec::new();
        }

        let total_weight: f64 = self.strengths.values().sum();
        let total_load: f64 = ctx.finger_load.values().sum();
        if total_weight <= 0.0 || total_load <= 0.0 {
            return Vec::new();
        }

        let mut sum_sq_excess = 0.0;
        for (finger, strength) in &self.strengths {
            let load = ctx.finger_load.get(finger).copied().unwrap_or(0.0);
            let fair_share = total_load * strength / total_weight;
            let excess = (load - fair_share).max(0.0);
            sum_sq_excess += excess * excess;
        }

        vec![Hit {
            category: "finger_load",
            label: "overload".to_string(),
            cost: sum_sq_excess * self.overload_weight,
        }]
    }
}
