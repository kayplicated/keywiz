//! Hand-territory synchrony: cross-hand row alignment.
//!
//! Drift-specific (not in oxey): when a bigram alternates hands,
//! the two hands end up at whatever rows the two keys happen to
//! be on. If both hands are on home row the hands move in
//! parallel; if one is at the top and the other at the bottom,
//! the hands are splitting vertical territory and the sustained
//! motion feels disjointed.
//!
//! Kay's layout investigation found that gallium puts the right
//! hand's common letters on home+top while the left hand's common
//! letters sit on home+bot; every alternation bigram therefore
//! tends to split rows. Drifter moves the right hand's common
//! letters to home+bot, so both hands share the same vertical
//! territory and move together. This rule scores that effect
//! directly.
//!
//! The rule fires on each adjacent pair inside the trigram
//! (positions 0-1 and 1-2). For each pair that alternates hands,
//! it applies a reward or penalty based on the row delta:
//!
//! * Both on home, delta 0   → `same_row_reward`
//! * Delta 1 (home↔top or home↔bot) → `one_row_penalty`
//! * Delta 2 (top↔bot)       → `two_row_penalty`
//!
//! Same-hand pairs contribute nothing here — they are the domain
//! of the roll / onehand / redirect rules.

use anyhow::Result;
use toml::Value;

use crate::trigram::config_util::read_f64;
use crate::trigram::context::TrigramContext;
use crate::trigram::rule::{RuleHit, TrigramRule};

pub struct HandTerritory {
    same_row_reward: f64,
    one_row_penalty: f64,
    two_row_penalty: f64,
}

impl HandTerritory {
    pub fn from_config(sub: Option<&Value>) -> Result<Self> {
        Ok(Self {
            same_row_reward: read_f64(sub, "same_row_reward", 0.5),
            one_row_penalty: read_f64(sub, "one_row_penalty", -0.3),
            two_row_penalty: read_f64(sub, "two_row_penalty", -1.0),
        })
    }

    /// Per-pair contribution given the two keys' logical rows.
    /// Rows use keywiz's convention: -1 top, 0 home, 1 bot.
    fn pair_cost(&self, row_a: i32, row_b: i32) -> f64 {
        let delta = (row_a - row_b).abs();
        match delta {
            0 => self.same_row_reward,
            1 => self.one_row_penalty,
            2 => self.two_row_penalty,
            // Drift only scores the 30-key alpha core where rows
            // are {-1, 0, 1}, so deltas >2 shouldn't appear. Treat
            // them as the 2-row penalty if they ever do.
            _ => self.two_row_penalty,
        }
    }
}

impl TrigramRule for HandTerritory {
    fn name(&self) -> &'static str { "hand_territory" }

    fn evaluate(&self, ctx: &TrigramContext) -> Option<RuleHit> {
        // Walk both adjacent pairs. Only alternation pairs contribute.
        let mut cost = 0.0;
        let mut contributed = false;

        for pair_i in 0..2 {
            if ctx.same_hand[pair_i] {
                continue;
            }
            contributed = true;
            cost += self.pair_cost(ctx.row(pair_i), ctx.row(pair_i + 1));
        }

        if !contributed {
            return None;
        }

        let [a, b, c] = ctx.chars;
        Some(RuleHit {
            category: "hand_territory",
            label: format!("{}{}{}", a, b, c),
            cost: ctx.freq * cost,
        })
    }
}
