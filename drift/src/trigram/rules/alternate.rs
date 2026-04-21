//! Hand-alternating trigrams (L-R-L or R-L-R).
//!
//! A small constant reward per alternating trigram. Alternation is
//! generally comfortable but doesn't flow as fast as a same-hand
//! roll; hence the smaller weight than rolls in the default config.

use anyhow::Result;
use toml::Value;

use crate::trigram::config_util::read_f64;
use crate::trigram::context::TrigramContext;
use crate::trigram::rule::{RuleHit, TrigramRule};

pub struct Alternate {
    weight: f64,
}

impl Alternate {
    pub fn from_config(sub: Option<&Value>) -> Result<Self> {
        Ok(Self {
            weight: read_f64(sub, "weight", 0.4),
        })
    }
}

impl TrigramRule for Alternate {
    fn name(&self) -> &'static str { "alternate" }

    fn evaluate(&self, ctx: &TrigramContext) -> Option<RuleHit> {
        if !ctx.is_alternating() {
            return None;
        }
        let [a, b, c] = ctx.chars;
        Some(RuleHit {
            category: "alternate",
            label: format!("{}{}{}", a, b, c),
            cost: ctx.freq * self.weight,
        })
    }
}
