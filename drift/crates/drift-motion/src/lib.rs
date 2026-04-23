//! Bigram geometric primitives.
//!
//! Pure functions that compute geometric facts about key pairs:
//! finger gap, row delta, roll direction, asymmetric-forward
//! exemption. Consumed by bigram-scope analyzers in drift-analyzers.
//!
//! This crate replaces the `Motion` enum of the pre-refactor drift:
//! classification and scoring are no longer fused. Analyzers read
//! a [`Geometry`] and decide independently whether it matches their
//! pattern and what to score. One geometric fact can feed multiple
//! analyzers without a central classifier having to decide who owns
//! it.

pub mod asymmetric;
pub mod cross_row;
pub mod roll;

pub use asymmetric::{is_forward_exempt, AsymmetricRules};
pub use cross_row::{cross_row_kind, CrossRowKind};
pub use roll::{roll_direction, RollDirection};

use drift_core::Key;

/// Raw geometric facts about a bigram. Analyzers read the fields
/// they care about and decide their own classification.
#[derive(Debug, Clone, Copy)]
pub struct Geometry {
    /// Both keys on the same hand.
    pub same_hand: bool,
    /// Both keys use the same finger.
    pub same_finger: bool,
    /// Absolute finger-column distance (0 = same finger, 3 =
    /// pinky-to-index). Meaningless across hands; 0 in that case.
    pub finger_gap: u8,
    /// Signed column delta in physical key-units (`b.x - a.x`).
    pub dx: f64,
    /// Signed row delta in physical key-units (`b.y - a.y`).
    pub dy: f64,
    /// Signed logical-row delta. `0` = same row. Positive = `b` is
    /// physically below `a` (toward bottom).
    pub row_delta: i32,
}

/// Compute geometric facts for a bigram.
pub fn geometry(a: &Key, b: &Key) -> Geometry {
    let same_hand = a.finger.same_hand(b.finger);
    let same_finger = a.finger == b.finger;
    let finger_gap = if same_hand {
        a.finger.column_distance(b.finger).unwrap_or(0)
    } else {
        0
    };
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let row_delta = row_index(b.row) - row_index(a.row);
    Geometry {
        same_hand,
        same_finger,
        finger_gap,
        dx,
        dy,
        row_delta,
    }
}

/// Map a named row to a logical integer index for arithmetic.
/// Top = -1, Home = 0, Bottom = 1, Number = -2, Extra(n) = 2 + n.
/// Unknown future row variants fall through to 0 (home-equivalent);
/// analyzers that care about a specific new row should pattern-match
/// directly on `Row` rather than going through `row_index`.
fn row_index(row: drift_core::Row) -> i32 {
    use drift_core::Row;
    match row {
        Row::Number => -2,
        Row::Top => -1,
        Row::Home => 0,
        Row::Bottom => 1,
        Row::Extra(n) => 2 + i32::from(n),
        _ => 0,
    }
}
