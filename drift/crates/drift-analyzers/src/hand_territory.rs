//! Hand-territory analyzer — cross-hand row-synchrony score.
//!
//! For each adjacent cross-hand pair inside the trigram, rewards
//! or penalizes based on the row delta between the two hands.
//! Captures the structural property that matched row territory
//! between hands makes alternation feel aligned; split territory
//! makes it feel disjointed.

use anyhow::Result;
use drift_analyzer::{f64_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Hit, Scope, Window};

use crate::row_util::row_index;

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "hand_territory",
        build: |cfg| Ok(Box::new(HandTerritory::from_config(cfg)?)),
    });
}

pub struct HandTerritory {
    pub same_row_reward: f64,
    pub one_row_penalty: f64,
    pub two_row_penalty: f64,
}

impl HandTerritory {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        Ok(Self {
            same_row_reward: f64_or(cfg, "same_row_reward", 0.5),
            one_row_penalty: f64_or(cfg, "one_row_penalty", -0.3),
            two_row_penalty: f64_or(cfg, "two_row_penalty", -1.0),
        })
    }

    fn pair_cost(&self, row_a: i32, row_b: i32) -> f64 {
        match (row_a - row_b).abs() {
            0 => self.same_row_reward,
            1 => self.one_row_penalty,
            2 => self.two_row_penalty,
            _ => self.two_row_penalty,
        }
    }
}

impl Analyzer for HandTerritory {
    fn name(&self) -> &'static str {
        "hand_territory"
    }

    fn scope(&self) -> Scope {
        Scope::Trigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let p = window.props;
        let mut cost = 0.0;
        let mut contributed = false;
        for i in 0..2 {
            if p.same_hand_pairs[i] {
                continue;
            }
            contributed = true;
            cost += self.pair_cost(row_index(p.rows[i]), row_index(p.rows[i + 1]));
        }
        if !contributed {
            return Vec::new();
        }

        let [a, b, c] = [window.chars[0], window.chars[1], window.chars[2]];
        vec![Hit {
            category: "hand_territory",
            label: format!("{a}{b}{c}"),
            cost: window.freq * cost,
        }]
    }
}
