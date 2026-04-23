//! A single physical key.
//!
//! `Key` is the physical-geometry record for one position on a
//! keyboard. Layouts reference keys by `KeyId`. `KeyId` is a newtype
//! around `String` so we can later add validation or interning
//! without touching every consumer.

use crate::{Finger, Row};

/// Which sub-column of a finger's reach a key belongs to.
///
/// Non-index fingers have only one column — [`FingerColumn::Outer`].
/// Index fingers own two columns on most keyboards: the outer one
/// (the natural resting position, farther from the thumb) and the
/// inner one (reached inward toward the thumb, across the central
/// gap on split boards). Analyzers that care about true vertical
/// SFBs vs. lateral same-finger column-crossings compare two keys'
/// `finger_column` values; analyzers that only care about which
/// finger owns a key (finger load, roll detection) can ignore it.
///
/// The terms follow keyboard convention: "inner" = closer to the
/// thumb, "outer" = farther from it. For ring, middle, and pinky
/// fingers there's no inner column, so they're always `Outer`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FingerColumn {
    /// The finger's main/outer column. This is the finger's
    /// natural resting column — farther from the thumb. For
    /// non-index fingers it's the only column; for index fingers
    /// it's what typists usually call "home index."
    Outer,
    /// The index finger's inner column — the one reached inward
    /// toward the thumb, across the central gap on split boards.
    /// Only meaningful for index fingers.
    Inner,
}

/// Stable identifier for a key position on a keyboard.
///
/// Currently a thin wrapper over `String`. The newtype lets us add
/// validation or interning later without source-churning every site
/// that refers to a key by id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyId(pub String);

impl KeyId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for KeyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One physical key on a keyboard. Geometry in key-units with `y`
/// growing away from the user (i.e. higher rows have larger y if
/// the board is laid out that way; boards may invert — consult
/// the loader's convention).
#[derive(Debug, Clone)]
pub struct Key {
    pub id: KeyId,
    /// Column index, signed. Negative = left hand, positive = right.
    /// Exact range depends on the keyboard.
    pub col: i32,
    /// Named row.
    pub row: Row,
    /// Physical x-position in key-units.
    pub x: f64,
    /// Physical y-position in key-units.
    pub y: f64,
    /// Finger assigned to this key.
    pub finger: Finger,
    /// Which column of the finger this key belongs to. For most
    /// keys this is [`FingerColumn::Primary`]; for index-center
    /// keys (inner-reach slots on split/col-stag boards) it's
    /// [`FingerColumn::IndexCenter`]. Analyzers that want to
    /// distinguish vertical SFBs from lateral column-crossings
    /// compare two keys' `finger_column` values.
    pub finger_column: FingerColumn,
}

impl Key {
    /// True iff both keys are on the same finger *and* the same
    /// sub-column. Vertical SFBs return true; lateral index-column
    /// motions return false.
    pub fn same_finger_column(&self, other: &Key) -> bool {
        self.finger == other.finger && self.finger_column == other.finger_column
    }
}
