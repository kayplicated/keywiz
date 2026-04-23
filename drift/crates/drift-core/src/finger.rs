//! Finger and hand identity.
//!
//! Drift's structural assumption: four fingers per hand on the alpha
//! grid (pinky, ring, middle, index). Thumbs are out of scope for
//! alpha scoring. Both enums are `#[non_exhaustive]` so future
//! variants (e.g. thumb-as-alpha) are additive rather than breaking.

/// Which hand a key belongs to.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hand {
    Left,
    Right,
}

/// One of the eight alpha-grid fingers.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Finger {
    LPinky,
    LRing,
    LMiddle,
    LIndex,
    RIndex,
    RMiddle,
    RRing,
    RPinky,
}

impl Finger {
    /// Which hand this finger belongs to.
    pub fn hand(self) -> Hand {
        use Finger::*;
        match self {
            LPinky | LRing | LMiddle | LIndex => Hand::Left,
            RIndex | RMiddle | RRing | RPinky => Hand::Right,
        }
    }

    /// True if both fingers are on the same hand.
    pub fn same_hand(self, other: Finger) -> bool {
        self.hand() == other.hand()
    }

    /// 0 = pinky, 3 = index. Relative to the finger's own hand.
    pub fn column(self) -> u8 {
        use Finger::*;
        match self {
            LPinky | RPinky => 0,
            LRing | RRing => 1,
            LMiddle | RMiddle => 2,
            LIndex | RIndex => 3,
        }
    }

    /// Column distance between two same-hand fingers. `None` if the
    /// fingers are on different hands.
    pub fn column_distance(self, other: Finger) -> Option<u8> {
        if !self.same_hand(other) {
            return None;
        }
        Some((self.column() as i8 - other.column() as i8).unsigned_abs())
    }
}
