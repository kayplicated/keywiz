//! Bigram motion classification.
//!
//! Given two keys and their physical positions on a keyboard, decide
//! what kind of motion their bigram represents. Classification is
//! consulted by the scorer to apply the right cost/reward.

use crate::config::AsymmetricRules;
use crate::keyboard::{Finger, Key};

/// Classification of a same-hand motion between two keys.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Motion {
    /// Alternation (different hands). No same-hand cost.
    Alternate,

    /// Same-finger repeat (same key twice). No motion cost.
    SameKey,

    /// Same-finger bigram — moves within the same column family
    /// on the same finger.
    Sfb { dy_abs: f64, dx_abs: f64 },

    /// Clean same-row roll across adjacent fingers.
    Roll { direction: RollDirection, dx: f64 },

    /// Same-row non-adjacent skip (e.g. index to ring directly).
    SameRowSkip { dx: f64, finger_gap: u8 },

    /// Adjacent-finger cross-row move (scissor-like), after applying
    /// any asymmetric-forward rules.
    CrossRow {
        kind: CrossRowKind,
        /// Signed dy (b.y - a.y). Positive = second key is physically
        /// farther from user (higher row).
        dy: f64,
        dx: f64,
    },

    /// Asymmetric-forward cross-row that was filtered to NOT be a
    /// scissor (outer finger naturally-extended). Reported as a
    /// neutral adjacent motion, no penalty.
    AdjacentForwardOk {
        dy: f64,
        dx: f64,
    },

    /// Non-adjacent, non-same-row: treat as weak same-hand motion.
    /// Covers stretches like pinky-to-middle across rows.
    Stretch {
        finger_gap: u8,
        dy: f64,
        dx: f64,
    },
}

/// Which direction a roll travels, from first to second key.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RollDirection {
    /// Toward the body center (outer to inner finger).
    Inward,
    /// Toward the body edge (inner to outer finger).
    Outward,
}

/// Which row-pair the cross-row motion spans.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CrossRowKind {
    /// Home to bottom or bottom to home. Flexion-dominant.
    Flexion,
    /// Home to top or top to home. Extension-dominant.
    Extension,
    /// Top to bottom or bottom to top. Spans both rows.
    FullCross,
}

/// Classify the motion of a bigram given the two key positions.
///
/// `asym` controls whether natural forward-resting shapes are
/// exempted from scissor classification.
pub fn classify(a: &Key, b: &Key, asym: &AsymmetricRules) -> Motion {
    if a.finger == b.finger && a.id == b.id {
        return Motion::SameKey;
    }

    if !a.finger.same_hand(b.finger) {
        return Motion::Alternate;
    }

    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let dx_abs = dx.abs();
    let dy_abs = dy.abs();

    if a.finger == b.finger {
        // Within-finger motion.
        return Motion::Sfb { dy_abs, dx_abs };
    }

    let finger_gap = a
        .finger
        .column_distance(b.finger)
        .expect("same_hand verified above");

    // Row classification by logical row index (top=-1, home=0, bot=1).
    let row_cross = a.row != b.row;
    let row_adjacent = row_cross && (a.row - b.row).abs() == 1;
    let row_full = row_cross && (a.row - b.row).abs() == 2;

    if !row_cross {
        // Same row, different fingers.
        if finger_gap == 1 {
            let direction = roll_direction(a.finger, b.finger);
            return Motion::Roll { direction, dx };
        }
        return Motion::SameRowSkip { dx, finger_gap };
    }

    if finger_gap == 1 && row_adjacent {
        // Adjacent-finger single-row cross. Scissor candidate.
        if asymmetric_forward_exempt(a, b, asym) {
            return Motion::AdjacentForwardOk { dy, dx };
        }
        let kind = cross_row_kind(a.row, b.row);
        return Motion::CrossRow { kind, dy, dx };
    }

    if finger_gap == 1 && row_full {
        return Motion::CrossRow {
            kind: CrossRowKind::FullCross,
            dy,
            dx,
        };
    }

    // Non-adjacent same-hand move across rows.
    Motion::Stretch {
        finger_gap,
        dy,
        dx,
    }
}

/// On any hand, going from a higher-column-index finger (more-inner,
/// closer to thumb) to a lower-column-index finger is outward.
/// Think: middle -> ring -> pinky = outward.
fn roll_direction(a: Finger, b: Finger) -> RollDirection {
    // Fingers are indexed 0=pinky..3=index. Going 3->2->1->0 = outward.
    use Finger::*;
    let idx = |f: Finger| match f {
        LPinky | RPinky => 0,
        LRing | RRing => 1,
        LMiddle | RMiddle => 2,
        LIndex | RIndex => 3,
    };
    if idx(a) < idx(b) {
        RollDirection::Inward
    } else {
        RollDirection::Outward
    }
}

/// Categorize a cross-row motion by which rows it spans.
fn cross_row_kind(row_a: i32, row_b: i32) -> CrossRowKind {
    let (lo, hi) = (row_a.min(row_b), row_a.max(row_b));
    match (lo, hi) {
        (-1, 0) => CrossRowKind::Extension, // top<->home
        (0, 1) => CrossRowKind::Flexion,    // home<->bot
        (-1, 1) => CrossRowKind::FullCross, // top<->bot
        _ => CrossRowKind::Flexion,         // shouldn't happen for alpha core
    }
}

/// Apply the "outer finger naturally pre-extended forward is fine"
/// rule: if the more-outer finger (closer to pinky) is at a larger
/// y (physically closer to user), the hand is in a natural resting
/// splay and the motion isn't a scissor.
///
/// y in keywiz keyboards grows downward from the number row, so
/// "physically closer to user" = larger y.
fn asymmetric_forward_exempt(a: &Key, b: &Key, asym: &AsymmetricRules) -> bool {
    let (outer, inner) = outer_inner(a, b);
    if outer.y < inner.y {
        // Outer finger is at smaller y = physically further from user
        // = more extended upward. This is the awkward twist the rule
        // targets: don't exempt.
        return false;
    }

    // Outer finger is forward (natural rest). Check whether the
    // relevant finger pair has the rule enabled.
    use Finger::*;
    match (outer.finger, inner.finger) {
        (LIndex, LMiddle) | (RIndex, RMiddle) => asym.index_middle_forward_ok,
        (LMiddle, LRing) | (RMiddle, RRing) => asym.middle_ring_forward_ok,
        (LRing, LPinky) | (RRing, RPinky) => asym.ring_pinky_forward_ok,
        _ => false,
    }
}

/// Given two same-hand keys, return `(outer, inner)` — outer is the
/// one closer to the pinky edge, inner is closer to the thumb.
fn outer_inner<'a>(a: &'a Key, b: &'a Key) -> (&'a Key, &'a Key) {
    use Finger::*;
    let col_weight = |f: Finger| match f {
        LPinky | RPinky => 0,
        LRing | RRing => 1,
        LMiddle | RMiddle => 2,
        LIndex | RIndex => 3,
    };
    if col_weight(a.finger) < col_weight(b.finger) {
        (a, b)
    } else {
        (b, a)
    }
}
