//! Context passed to aggregate-scope analyzers.
//!
//! Aggregate analyzers run once, after per-window passes. They see
//! whole-corpus rollups (per-char load, per-finger load) rather
//! than a single window. The pipeline populates these rollups
//! during the unigram pass and hands them off at aggregate time.

use std::collections::HashMap;

use drift_core::{Finger, Layout};

/// Context available to aggregate-scope analyzers.
pub struct AggregateContext<'a> {
    /// The layout being scored.
    pub layout: &'a Layout,

    /// Name of the corpus, for labeling only.
    pub corpus_name: &'a str,

    /// Per-character load across the corpus (as percentages).
    pub char_load: &'a HashMap<char, f64>,

    /// Per-finger load across the corpus (as percentages). Derived
    /// from `char_load` plus the layout.
    pub finger_load: &'a HashMap<Finger, f64>,
}
