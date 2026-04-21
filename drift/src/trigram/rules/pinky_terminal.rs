//! Penalty for trigrams that end on a pinky.
//!
//! Landing a 3-key sequence on the pinky is often unstable — the
//! preceding motion builds momentum on stronger fingers, then the
//! weakest finger has to arrest it on the final keystroke. Some
//! hands (Kay's in particular) find this consistently uncomfortable.
//!
//! The rule applies regardless of other rules; a trigram can be
//! both a roll (reward) and pinky-terminal (penalty) at once.

use anyhow::Result;
use toml::Value;

use crate::keyboard::Finger;
use crate::trigram::config_util::read_f64;
use crate::trigram::context::TrigramContext;
use crate::trigram::rule::{RuleHit, TrigramRule};

pub struct PinkyTerminal {
    penalty: f64,
}

impl PinkyTerminal {
    pub fn from_config(sub: Option<&Value>) -> Result<Self> {
        Ok(Self {
            penalty: read_f64(sub, "penalty", -0.5),
        })
    }
}

impl TrigramRule for PinkyTerminal {
    fn name(&self) -> &'static str { "pinky_terminal" }

    fn evaluate(&self, ctx: &TrigramContext) -> Option<RuleHit> {
        if !matches!(ctx.terminal_finger(), Finger::LPinky | Finger::RPinky) {
            return None;
        }
        // Only penalize same-hand motions ending on pinky. Alternating
        // trigrams ending on pinky aren't a "landing with momentum"
        // — they're an isolated pinky press.
        if !ctx.all_same_hand {
            return None;
        }
        let [a, b, c] = ctx.chars;
        Some(RuleHit {
            category: "pinky_terminal",
            label: format!("{}{}{}", a, b, c),
            cost: ctx.freq * self.penalty,
        })
    }
}
