//! Trigram alternation analyzer.
//!
//! Fires on trigrams where at least one adjacent pair alternates
//! hands. Splits the reward:
//!
//! - **Strict alternation** (L-R-L or R-L-R): both pairs cross-hand.
//!   Highest reward — the hands trade every keystroke, which is
//!   the most rhythmic same-rate pattern.
//! - **Partial alternation** (L-L-R, R-R-L, L-R-R, R-L-L): exactly
//!   one pair cross-hand. Still a hand-switch, but with one
//!   same-hand beat attached. Reduced reward.
//!
//! Set `partial_weight = 0.0` to score only strict alternation
//! (pre-partial-alternation behavior). Set `partial_weight` equal
//! to `weight` to treat partial and strict identically.

use anyhow::Result;
use drift_analyzer::{f64_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Hit, Scope, Window};

use crate::trigram_util::is_alternating;

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "alternate",
        build: |cfg| Ok(Box::new(Alternate::from_config(cfg)?)),
    });
}

pub struct Alternate {
    pub weight: f64,
    pub partial_weight: f64,
}

impl Alternate {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        Ok(Self {
            weight: f64_or(cfg, "weight", 0.4),
            partial_weight: f64_or(cfg, "partial_weight", 0.15),
        })
    }
}

impl Analyzer for Alternate {
    fn name(&self) -> &'static str {
        "alternate"
    }

    fn scope(&self) -> Scope {
        Scope::Trigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let p = window.props;
        let [a, b, c] = [window.chars[0], window.chars[1], window.chars[2]];

        // Count cross-hand adjacent pairs (0, 1, or 2).
        let cross_count =
            (!p.same_hand_pairs[0]) as u8 + (!p.same_hand_pairs[1]) as u8;

        if cross_count == 2 {
            // Strict alternation: L-R-L or R-L-R.
            if is_alternating(p) {
                return vec![Hit {
                    category: "alternate",
                    label: format!("{a}{b}{c}"),
                    cost: window.freq * self.weight,
                }];
            }
            Vec::new()
        } else if cross_count == 1 && self.partial_weight != 0.0 {
            // Partial alternation: one hand-switch across three keys.
            vec![Hit {
                category: "alternate",
                label: format!("partial {a}{b}{c}"),
                cost: window.freq * self.partial_weight,
            }]
        } else {
            Vec::new()
        }
    }
}
