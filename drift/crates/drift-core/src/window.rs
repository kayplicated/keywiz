//! The `Window` passed to analyzers during a scoring pass.
//!
//! A window bundles a slice of the corpus (chars + frequency) with
//! the resolved key positions for those chars. Precomputed geometric
//! properties live in `WindowProps`, built once per window and
//! shared across all analyzers that see it.

use crate::{Key, Row};

/// A frequency-weighted slice of the corpus paired with its
/// resolved key positions on the current layout.
pub struct Window<'a> {
    /// Characters in this window, in corpus order.
    pub chars: &'a [char],
    /// Resolved key per character (same length as `chars`).
    pub keys: &'a [&'a Key],
    /// Percentage frequency of this exact sequence in the corpus.
    pub freq: f64,
    /// Shared precomputed geometry.
    pub props: &'a WindowProps,
}

/// Geometric properties derived once per window, shared across all
/// analyzers that see it. Populated by the pipeline before calling
/// analyzers.
pub struct WindowProps {
    /// `same_hand_pairs[i]` is true iff `keys[i]` and `keys[i+1]`
    /// are on the same hand. Length is `chars.len() - 1`.
    pub same_hand_pairs: Vec<bool>,

    /// True iff every key in the window is on the same hand.
    pub all_same_hand: bool,

    /// Finger-column index (0 = pinky, 3 = index) for each key.
    /// Length matches `chars`.
    pub finger_columns: Vec<u8>,

    /// Named row for each key. Length matches `chars`.
    pub rows: Vec<Row>,
}
