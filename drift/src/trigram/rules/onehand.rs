//! Same-hand trigram that isn't cleanly a roll.
//!
//! "Onehand" = all three letters on the same hand, fingers are
//! monotonic in a direction but not strictly adjacent-stepped (so
//! [`crate::trigram::rules::roll`] already handled roll3 and
//! roll3-skip; this covers other same-direction sequences).
//!
//! Practically: onehand is a weaker reward than a clean roll,
//! because the hand stays engaged without the rhythmic stepping.

use anyhow::Result;
use toml::Value;

use crate::trigram::config_util::read_f64;
use crate::trigram::context::TrigramContext;
use crate::trigram::rule::{RuleHit, TrigramRule};

pub struct Onehand {
    weight: f64,
}

impl Onehand {
    pub fn from_config(sub: Option<&Value>) -> Result<Self> {
        Ok(Self {
            weight: read_f64(sub, "weight", 1.0),
        })
    }
}

impl TrigramRule for Onehand {
    fn name(&self) -> &'static str { "onehand" }

    fn evaluate(&self, ctx: &TrigramContext) -> Option<RuleHit> {
        if !ctx.all_same_hand {
            return None;
        }

        // Skip cases that roll rules already handle (full and skip).
        use crate::trigram::context::RollDir::*;
        if ctx.is_roll3(Inward) || ctx.is_roll3(Outward)
            || ctx.is_roll3_skip(Inward) || ctx.is_roll3_skip(Outward)
        {
            return None;
        }

        // Skip redirects — they get their own rule.
        if ctx.is_redirect() {
            return None;
        }

        // At this point it's same-hand, monotonic-ish, no redirects,
        // not a clean stepping roll. Count it as generic onehand.
        let [a, b, c] = ctx.chars;
        Some(RuleHit {
            category: "onehand",
            label: format!("{}{}{}", a, b, c),
            cost: ctx.freq * self.weight,
        })
    }
}
