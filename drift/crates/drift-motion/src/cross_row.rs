//! Cross-row kind classification.
//!
//! Maps a pair of rows to a direction category (flexion =
//! home↔bottom, extension = home↔top, full cross = top↔bottom).
//! Analyzers use this to apply direction-dependent weights.

use drift_core::Row;

/// Which row-pair a cross-row bigram spans.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossRowKind {
    /// Home ↔ Bottom.
    Flexion,
    /// Home ↔ Top.
    Extension,
    /// Top ↔ Bottom.
    FullCross,
    /// Something else (e.g. number row, extra rows). Analyzers that
    /// care only about the alpha core can ignore these.
    Other,
}

/// Classify a cross-row pair of [`Row`]s.
pub fn cross_row_kind(a: Row, b: Row) -> CrossRowKind {
    use Row::*;
    match (a, b) {
        (Home, Bottom) | (Bottom, Home) => CrossRowKind::Flexion,
        (Home, Top) | (Top, Home) => CrossRowKind::Extension,
        (Top, Bottom) | (Bottom, Top) => CrossRowKind::FullCross,
        _ => CrossRowKind::Other,
    }
}
