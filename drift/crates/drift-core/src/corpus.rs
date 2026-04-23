//! The `CorpusSource` trait — drift's abstraction over n-gram
//! frequency data.
//!
//! Analyzers depend on this trait, not any concrete struct. The
//! default implementation (in drift-corpus) loads oxey's JSON
//! format into memory. Alternative implementations — streaming,
//! memory-mapped, derived-on-demand — can be swapped in without
//! touching analyzer code.

/// Frequency-data source. All frequencies are percentages (0..=100)
/// of total typing, not raw counts. A source that doesn't have data
/// for a given lookup returns 0.0.
pub trait CorpusSource: Send + Sync {
    /// Human-readable name, used in reports.
    fn name(&self) -> &str;

    /// Frequency of a single character.
    fn char_freq(&self, c: char) -> f64;

    /// Frequency of a bigram.
    fn bigram_freq(&self, a: char, b: char) -> f64;

    /// Frequency of a trigram.
    fn trigram_freq(&self, a: char, b: char, c: char) -> f64;

    /// Frequency of an arbitrary-length n-gram. Sources that don't
    /// store data for length `chars.len()` may derive an estimate
    /// or return 0.0 — callers should not rely on exact values for
    /// higher n unless the source advertises support (see
    /// [`max_ngram_length`]).
    fn ngram_freq(&self, chars: &[char]) -> f64;

    /// Iterate over (char, frequency) entries.
    fn iter_chars<'a>(&'a self) -> Box<dyn Iterator<Item = (char, f64)> + 'a>;

    /// Iterate over (bigram, frequency) entries.
    fn iter_bigrams<'a>(&'a self) -> Box<dyn Iterator<Item = ((char, char), f64)> + 'a>;

    /// Iterate over (trigram, frequency) entries.
    fn iter_trigrams<'a>(&'a self) -> Box<dyn Iterator<Item = ((char, char, char), f64)> + 'a>;

    /// Iterate over n-grams of the given length. Implementations
    /// may stream, derive, or error if the length isn't supported.
    fn iter_ngrams<'a>(&'a self, n: usize) -> Box<dyn Iterator<Item = (Vec<char>, f64)> + 'a>;

    /// Iterate over skipgrams with a given gap. A skipgram of
    /// `gap = n` is a char pair `(a, b)` where the source text had
    /// `n` characters between `a` and `b`. `gap = 1` corresponds to
    /// the oxey `skipgrams` field, `gap = 2` to `skipgrams2`, etc.
    ///
    /// Sources without skipgram data return an empty iterator.
    fn iter_skipgrams<'a>(
        &'a self,
        gap: usize,
    ) -> Box<dyn Iterator<Item = ((char, char), f64)> + 'a> {
        let _ = gap;
        Box::new(std::iter::empty())
    }

    /// The largest n-gram length this source can provide accurate
    /// frequencies for. Anything beyond this may still return
    /// values via derivation, but precision isn't guaranteed.
    fn max_ngram_length(&self) -> usize {
        3
    }
}
