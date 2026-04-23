//! The `Pipeline` — ordered, scope-grouped set of analyzer instances.
//!
//! A pipeline is built from config and handed to drift-score. It
//! retains the user's ordering so reports and hit iteration are
//! stable across runs, and it exposes a scope-grouped view so
//! drift-score can do one pass per window shape.

use drift_core::Scope;

use crate::Analyzer;

/// An ordered collection of analyzers.
pub struct Pipeline {
    analyzers: Vec<Box<dyn Analyzer>>,
}

impl Pipeline {
    /// Iterate over all analyzers in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = &dyn Analyzer> {
        self.analyzers.iter().map(|a| a.as_ref())
    }

    /// True if no analyzers are enabled.
    pub fn is_empty(&self) -> bool {
        self.analyzers.is_empty()
    }

    /// Number of analyzers.
    pub fn len(&self) -> usize {
        self.analyzers.len()
    }

    /// All scopes used by enabled analyzers, in no particular order.
    /// drift-score uses this to know which passes to run.
    pub fn scopes(&self) -> Vec<Scope> {
        let mut scopes: Vec<Scope> = self.analyzers.iter().map(|a| a.scope()).collect();
        scopes.sort_by_key(|s| scope_sort_key(*s));
        scopes.dedup();
        scopes
    }

    /// Analyzers matching a given scope, in order.
    pub fn for_scope(&self, scope: Scope) -> impl Iterator<Item = &dyn Analyzer> {
        self.analyzers
            .iter()
            .filter(move |a| a.scope() == scope)
            .map(|a| a.as_ref())
    }
}

/// Stable sort key for scopes. Drives pass ordering in drift-score:
/// Unigram, Bigram, Trigram, Skipgram(gap) ascending, Ngram(n)
/// ascending, then Aggregate. Unknown future scope variants sort
/// after `Aggregate`, so they execute last and do no damage to
/// existing passes.
///
/// Skipgrams come before Ngrams of the same arithmetic weight
/// because they produce length-2 windows — conceptually closer to
/// Bigram than to a higher-order n-gram.
fn scope_sort_key(s: Scope) -> u32 {
    match s {
        Scope::Unigram => 0,
        Scope::Bigram => 1,
        Scope::Trigram => 2,
        Scope::Skipgram(gap) => 10 + gap as u32,
        Scope::Ngram(n) => 100 + n as u32,
        Scope::Aggregate => u32::MAX - 1,
        _ => u32::MAX,
    }
}

/// Incremental builder for `Pipeline`. Keeps insertion order and
/// can later grow validation / conflict checks without changing
/// the public `Pipeline` API.
pub struct PipelineBuilder {
    analyzers: Vec<Box<dyn Analyzer>>,
}

impl PipelineBuilder {
    pub fn new() -> Self {
        Self {
            analyzers: Vec::new(),
        }
    }

    pub fn push(&mut self, analyzer: Box<dyn Analyzer>) {
        self.analyzers.push(analyzer);
    }

    pub fn build(self) -> Pipeline {
        Pipeline {
            analyzers: self.analyzers,
        }
    }
}

impl Default for PipelineBuilder {
    fn default() -> Self {
        Self::new()
    }
}
