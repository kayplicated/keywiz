//! Penalty for trigrams that cascade across all three rows.
//!
//! Drift-specific (not in oxey): when a same-hand trigram visits
//! top, home, and bottom rows in sequence — e.g. `key` on drifter
//! where k=top, e=home, y=bot — the hand has to reshape at every
//! keystroke. This is the "roller coaster" effect Kay complained
//! about in drilling.
//!
//! The rule fires when {top, home, bot} are all covered across
//! the three keys on the same hand.

use anyhow::Result;
use std::collections::HashSet;
use toml::Value;

use crate::trigram::config_util::read_f64;
use crate::trigram::context::TrigramContext;
use crate::trigram::rule::{RuleHit, TrigramRule};

pub struct RowCascade {
    weight: f64,
}

impl RowCascade {
    pub fn from_config(sub: Option<&Value>) -> Result<Self> {
        Ok(Self {
            weight: read_f64(sub, "weight", -3.0),
        })
    }
}

impl TrigramRule for RowCascade {
    fn name(&self) -> &'static str { "row_cascade" }

    fn evaluate(&self, ctx: &TrigramContext) -> Option<RuleHit> {
        if !ctx.all_same_hand {
            return None;
        }

        let mut rows: HashSet<i32> = HashSet::with_capacity(3);
        for i in 0..3 {
            rows.insert(ctx.row(i));
        }

        // Only all three alpha rows count.
        if !(rows.contains(&-1) && rows.contains(&0) && rows.contains(&1)) {
            return None;
        }

        let [a, b, c] = ctx.chars;
        Some(RuleHit {
            category: "row_cascade",
            label: format!("{}{}{}", a, b, c),
            cost: ctx.freq * self.weight,
        })
    }
}
