//! Trigram classification and scoring.
//!
//! Drift scores trigrams through a set of pluggable rules. Each
//! rule is a self-contained module implementing [`TrigramRule`];
//! the dispatcher calls every enabled rule against every trigram
//! and sums their contributions.
//!
//! Adding a new rule means creating a new file under `rules/` and
//! listing it in `[trigram] rules` in drift.toml. Rules are
//! additive: multiple rules can hit the same trigram, each
//! contributing an independent cost or reward.

pub mod config_util;
pub mod context;
pub mod registry;
pub mod rule;
pub mod rules;

pub use context::TrigramContext;
pub use registry::{build_pipeline, TrigramPipeline};
