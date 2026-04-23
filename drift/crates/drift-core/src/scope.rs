//! Analyzer scope — what shape of window an analyzer consumes.
//!
//! The pipeline dispatches one pass per unique scope. Analyzers of
//! the same scope share a pass.

/// What input shape an analyzer consumes.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// One character at a time, paired with its frequency.
    /// Used by analyzers that only need per-char aggregates
    /// (row distribution, per-finger load).
    Unigram,

    /// Adjacent char pair.
    Bigram,

    /// Adjacent char triple.
    Trigram,

    /// Fixed-length window of `n` characters (n > 3). The pipeline
    /// runs one pass per distinct length requested by enabled
    /// analyzers.
    Ngram(usize),

    /// Character pair with `gap` characters skipped between them.
    /// `Skipgram(1)` = chars at positions i and i+2 (one skipped),
    /// `Skipgram(2)` = i and i+3, etc.
    ///
    /// The window delivered to analyzers has length 2: the two non-
    /// skipped chars. Useful for same-finger-at-distance patterns
    /// and for rolls/alternation over a short pause that a regular
    /// bigram scope can't see.
    Skipgram(usize),

    /// Runs once after all per-window passes complete. Aggregate
    /// analyzers receive an `AggregateContext` with whole-corpus
    /// rollups rather than a window.
    Aggregate,
}
