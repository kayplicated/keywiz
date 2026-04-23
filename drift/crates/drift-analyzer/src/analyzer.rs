//! The `Analyzer` trait.

use drift_core::{Hit, Scope, Window};

use crate::AggregateContext;

/// A pluggable scoring module.
///
/// Analyzers are constructed once at pipeline-build time and then
/// called many times — once per window in the scoring pass that
/// matches their scope (or once via `evaluate_aggregate` for
/// aggregate-scope analyzers). They should be stateless with respect
/// to the pipeline; all state lives in the constructor-captured
/// configuration.
///
/// # Dependency declaration
///
/// `dependencies` tells the delta-scoring machinery which chars in
/// a window the analyzer's output depends on. The default is
/// conservative (the full window). Analyzers whose output depends
/// on a narrower subset can override to enable finer-grained delta
/// updates during simulated-annealing runs. Getting this wrong
/// produces incorrect delta scores, so err on the side of the
/// default unless the narrower set is obviously correct.
pub trait Analyzer: Send + Sync {
    /// Stable identifier. Matches the analyzer's entry in the
    /// config's enabled-rules list.
    fn name(&self) -> &'static str;

    /// Which window shape this analyzer consumes.
    fn scope(&self) -> Scope;

    /// Evaluate one window. Return zero or more hits.
    ///
    /// Called for windows matching `self.scope()`. Aggregate-scope
    /// analyzers receive an empty window from the pipeline; they
    /// should implement `evaluate_aggregate` instead.
    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let _ = window;
        Vec::new()
    }

    /// Evaluate against whole-corpus aggregates. Called once, after
    /// all per-window passes complete. Only meaningful for analyzers
    /// with `scope() == Scope::Aggregate`.
    fn evaluate_aggregate(&self, ctx: &AggregateContext) -> Vec<Hit> {
        let _ = ctx;
        Vec::new()
    }

    /// Which chars in `window` this analyzer's output depends on.
    /// Default: all of them. Override for narrower dependency sets
    /// (used by delta scoring to skip re-evaluation when a char-
    /// swap doesn't touch the window's dependencies).
    fn dependencies(&self, window: &Window) -> Vec<char> {
        window.chars.to_vec()
    }
}
