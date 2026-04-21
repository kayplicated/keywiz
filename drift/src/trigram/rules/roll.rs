//! Inward and outward 3-finger roll rules.
//!
//! A "roll3" is a same-hand trigram where the three fingers step
//! monotonically in one direction (inward = toward thumb, outward
//! = toward pinky). The rule also matches "roll3-skip" where one
//! step jumps a finger (pinky → ring → index); skip rolls score
//! at a configurable fraction of full rolls.
//!
//! Both rules support a `end_on_pinky_multiplier` so the reward
//! can be de-weighted when the terminal finger is the pinky (some
//! hands find this landing unstable).

use anyhow::Result;
use toml::Value;

use crate::keyboard::Finger;
use crate::trigram::config_util::read_f64;
use crate::trigram::context::{RollDir, TrigramContext};
use crate::trigram::rule::{RuleHit, TrigramRule};

/// Shared config for inward and outward roll rules.
struct RollConfig {
    weight: f64,
    skip_multiplier: f64,
    end_on_pinky_multiplier: f64,
}

impl RollConfig {
    fn from(sub: Option<&Value>, default_weight: f64) -> Result<Self> {
        Ok(Self {
            weight: read_f64(sub, "weight", default_weight),
            skip_multiplier: read_f64(sub, "skip_multiplier", 0.7),
            end_on_pinky_multiplier: read_f64(sub, "end_on_pinky_multiplier", 1.0),
        })
    }
}

/// Three-finger inward roll (pinky → ring → middle, etc).
pub struct InwardRoll {
    cfg: RollConfig,
}

impl InwardRoll {
    pub fn from_config(sub: Option<&Value>) -> Result<Self> {
        Ok(Self { cfg: RollConfig::from(sub, 3.0)? })
    }
}

impl TrigramRule for InwardRoll {
    fn name(&self) -> &'static str { "inward_roll" }

    fn evaluate(&self, ctx: &TrigramContext) -> Option<RuleHit> {
        classify_roll(ctx, RollDir::Inward, &self.cfg, "inward_roll")
    }
}

/// Three-finger outward roll (index → middle → ring, etc).
pub struct OutwardRoll {
    cfg: RollConfig,
}

impl OutwardRoll {
    pub fn from_config(sub: Option<&Value>) -> Result<Self> {
        Ok(Self { cfg: RollConfig::from(sub, 2.5)? })
    }
}

impl TrigramRule for OutwardRoll {
    fn name(&self) -> &'static str { "outward_roll" }

    fn evaluate(&self, ctx: &TrigramContext) -> Option<RuleHit> {
        classify_roll(ctx, RollDir::Outward, &self.cfg, "outward_roll")
    }
}

/// Classify a trigram as full, skip, or no-roll in the given
/// direction and return a hit if applicable.
fn classify_roll(
    ctx: &TrigramContext,
    dir: RollDir,
    cfg: &RollConfig,
    category: &'static str,
) -> Option<RuleHit> {
    let mut kind_multiplier = None;
    if ctx.is_roll3(dir) {
        kind_multiplier = Some(1.0);
    } else if ctx.is_roll3_skip(dir) {
        kind_multiplier = Some(cfg.skip_multiplier);
    }
    let kind_mult = kind_multiplier?;

    // Pinky-terminal multiplier: applies when the third finger is
    // a pinky.
    let terminal_mult = if is_pinky(ctx.terminal_finger()) {
        cfg.end_on_pinky_multiplier
    } else {
        1.0
    };

    let cost = ctx.freq * cfg.weight * kind_mult * terminal_mult;
    let [a, b, c] = ctx.chars;
    Some(RuleHit {
        category,
        label: format!("{}{}{}", a, b, c),
        cost,
    })
}

fn is_pinky(f: Finger) -> bool {
    matches!(f, Finger::LPinky | Finger::RPinky)
}

