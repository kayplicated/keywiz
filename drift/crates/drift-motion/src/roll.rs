//! Roll-direction classification.

use drift_core::Finger;

/// Direction a roll travels across adjacent fingers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollDirection {
    /// Toward the thumb edge (outer finger → inner finger).
    Inward,
    /// Toward the pinky edge (inner finger → outer finger).
    Outward,
}

/// Direction of a same-hand finger sequence from `a` to `b`.
/// Returns `None` for cross-hand or same-finger pairs.
pub fn roll_direction(a: Finger, b: Finger) -> Option<RollDirection> {
    if !a.same_hand(b) || a.column() == b.column() {
        return None;
    }
    if a.column() < b.column() {
        Some(RollDirection::Inward)
    } else {
        Some(RollDirection::Outward)
    }
}
