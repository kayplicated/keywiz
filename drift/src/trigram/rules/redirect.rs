//! Same-hand trigrams where the direction flips at the middle key.
//!
//! A redirect is: all same hand, and `sign(col1-col0) != sign(col2-col1)`.
//! Fingers go one direction then reverse. E.g. `pol` on a
//! `p|o|l|...` layout = inner→outer→inner. These feel like a
//! stutter.
//!
//! A "bad redirect" is a redirect where none of the three fingers
//! is the index finger — the hand has no strong anchor to pivot on.

use anyhow::Result;
use toml::Value;

use crate::keyboard::Finger;
use crate::trigram::config_util::read_f64;
use crate::trigram::context::TrigramContext;
use crate::trigram::rule::{RuleHit, TrigramRule};

/// Any same-hand direction flip.
pub struct Redirect {
    weight: f64,
}

impl Redirect {
    pub fn from_config(sub: Option<&Value>) -> Result<Self> {
        Ok(Self {
            weight: read_f64(sub, "weight", -3.0),
        })
    }
}

impl TrigramRule for Redirect {
    fn name(&self) -> &'static str { "redirect" }

    fn evaluate(&self, ctx: &TrigramContext) -> Option<RuleHit> {
        if !ctx.is_redirect() {
            return None;
        }
        // If any finger is index, let the "regular" redirect tag it.
        // Bad redirects (no index) are handled by the bad_redirect rule.
        if !has_index(ctx) {
            return None;
        }
        let [a, b, c] = ctx.chars;
        Some(RuleHit {
            category: "redirect",
            label: format!("{}{}{}", a, b, c),
            cost: ctx.freq * self.weight,
        })
    }
}

/// Redirect with no index finger involved.
pub struct BadRedirect {
    weight: f64,
}

impl BadRedirect {
    pub fn from_config(sub: Option<&Value>) -> Result<Self> {
        Ok(Self {
            weight: read_f64(sub, "weight", -5.0),
        })
    }
}

impl TrigramRule for BadRedirect {
    fn name(&self) -> &'static str { "bad_redirect" }

    fn evaluate(&self, ctx: &TrigramContext) -> Option<RuleHit> {
        if !ctx.is_redirect() {
            return None;
        }
        if has_index(ctx) {
            return None;
        }
        let [a, b, c] = ctx.chars;
        Some(RuleHit {
            category: "bad_redirect",
            label: format!("{}{}{}", a, b, c),
            cost: ctx.freq * self.weight,
        })
    }
}

fn has_index(ctx: &TrigramContext) -> bool {
    for i in 0..3 {
        if matches!(ctx.keys[i].finger, Finger::LIndex | Finger::RIndex) {
            return true;
        }
    }
    false
}
