//! Named rows.
//!
//! Analyzers pattern-match on `Row` rather than raw integers so they
//! stay readable and survive changes to row-index conventions. The
//! enum is `#[non_exhaustive]` so future variants (extra alpha rows
//! on larger boards) are additive.

/// A logical row on the keyboard.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Row {
    /// Number row (above top).
    Number,
    /// Top alpha row.
    Top,
    /// Home alpha row.
    Home,
    /// Bottom alpha row.
    Bottom,
    /// Additional alpha rows below Bottom, indexed from 0 for the
    /// first extra. Used by boards with more than three alpha rows.
    Extra(u8),
}

impl Row {
    /// True if this row is part of the alpha core (Top/Home/Bottom).
    pub fn is_alpha(self) -> bool {
        matches!(self, Row::Top | Row::Home | Row::Bottom)
    }
}
