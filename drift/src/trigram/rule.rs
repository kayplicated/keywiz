//! The [`TrigramRule`] trait.
//!
//! A rule reads a trigram + its geometric context and optionally
//! produces a [`RuleHit`]. Hits accumulate additively into the
//! layout's score.

use super::context::TrigramContext;

/// One contribution from one rule.
#[derive(Debug, Clone)]
pub struct RuleHit {
    /// Short category name for aggregation in the report.
    pub category: &'static str,
    /// Human-readable label for per-trigram display.
    pub label: String,
    /// Signed contribution to total score. Negative = penalty.
    pub cost: f64,
}

/// A pluggable trigram scoring rule.
///
/// Rules are stateless given their config (constructor). The
/// dispatcher calls [`TrigramRule::evaluate`] once per trigram.
pub trait TrigramRule: Send + Sync {
    /// Rule identifier, matches the string in `[trigram] rules`.
    /// Reserved for future introspection / debugging output.
    #[allow(dead_code)]
    fn name(&self) -> &'static str;

    /// Evaluate a trigram. Return `None` if the rule doesn't apply;
    /// return `Some(hit)` with the signed contribution if it does.
    fn evaluate(&self, ctx: &TrigramContext) -> Option<RuleHit>;
}
