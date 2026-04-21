//! Reward for trigrams whose row pattern favors flexion.
//!
//! Drift-specific (not in oxey): if a same-hand trigram stays on
//! home or the bottom row, and any cross-row motion is home→bot
//! (flexion) rather than home→top (extension), the whole trigram
//! executes with the hand curling rather than reaching. On col-stag
//! this feels genuinely faster and less tiring.
//!
//! The rule matches only when:
//! - all three keys are on the same hand
//! - all rows are in {home, bot} (no top-row letters at all)
//! - at least one cross-row step occurs (otherwise already rewarded
//!   as a same-row roll/onehand by other rules)

use anyhow::Result;
use toml::Value;

use crate::trigram::config_util::read_f64;
use crate::trigram::context::TrigramContext;
use crate::trigram::rule::{RuleHit, TrigramRule};

pub struct FlexionCascade {
    weight: f64,
}

impl FlexionCascade {
    pub fn from_config(sub: Option<&Value>) -> Result<Self> {
        Ok(Self {
            weight: read_f64(sub, "weight", 1.5),
        })
    }
}

impl TrigramRule for FlexionCascade {
    fn name(&self) -> &'static str { "flexion_cascade" }

    fn evaluate(&self, ctx: &TrigramContext) -> Option<RuleHit> {
        if !ctx.all_same_hand {
            return None;
        }

        // No top-row letters.
        for i in 0..3 {
            if ctx.row(i) == -1 {
                return None;
            }
        }

        // Must have at least one row transition (otherwise it's
        // a same-row sequence already handled by rolls).
        let row_varies = ctx.row(0) != ctx.row(1) || ctx.row(1) != ctx.row(2);
        if !row_varies {
            return None;
        }

        let [a, b, c] = ctx.chars;
        Some(RuleHit {
            category: "flexion_cascade",
            label: format!("{}{}{}", a, b, c),
            cost: ctx.freq * self.weight,
        })
    }
}
