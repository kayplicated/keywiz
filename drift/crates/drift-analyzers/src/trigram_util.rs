//! Shared trigram pattern predicates.
//!
//! Small helpers that multiple trigram analyzers reuse to test
//! finger-column patterns. None of them do scoring — they only
//! read `WindowProps::finger_columns` and return bools.

use drift_core::WindowProps;

/// True if finger columns step monotonically inward (cols increase),
/// each step adjacent (+1).
pub fn is_roll3_inward(props: &WindowProps) -> bool {
    if !props.all_same_hand {
        return false;
    }
    let c = &props.finger_columns;
    c[0] + 1 == c[1] && c[1] + 1 == c[2]
}

/// True if finger columns step monotonically outward (cols decrease),
/// each step adjacent (-1).
pub fn is_roll3_outward(props: &WindowProps) -> bool {
    if !props.all_same_hand {
        return false;
    }
    let c = &props.finger_columns;
    c[0] == c[1] + 1 && c[1] == c[2] + 1
}

/// Inward-roll-with-skip: monotonic inward, one step of size 2.
pub fn is_roll3_inward_skip(props: &WindowProps) -> bool {
    if !props.all_same_hand {
        return false;
    }
    let (d1, d2) = steps(props);
    d1 > 0 && d2 > 0 && (d1 + d2 <= 3) && (d1 == 2 || d2 == 2) && d1 != d2
}

/// Outward-roll-with-skip: monotonic outward, one step of size 2.
pub fn is_roll3_outward_skip(props: &WindowProps) -> bool {
    if !props.all_same_hand {
        return false;
    }
    let (d1, d2) = steps(props);
    d1 < 0 && d2 < 0 && (d1.abs() + d2.abs() <= 3) && (d1 == -2 || d2 == -2) && d1 != d2
}

/// True if the trigram alternates hands (L-R-L or R-L-R).
pub fn is_alternating(props: &WindowProps) -> bool {
    !props.same_hand_pairs[0] && !props.same_hand_pairs[1]
}

/// True if the trigram is same-hand with a direction flip at the
/// middle key.
pub fn is_redirect(props: &WindowProps) -> bool {
    if !props.all_same_hand {
        return false;
    }
    let (d1, d2) = steps(props);
    d1 != 0 && d2 != 0 && d1.signum() != d2.signum()
}

fn steps(props: &WindowProps) -> (i8, i8) {
    let c = &props.finger_columns;
    (c[1] as i8 - c[0] as i8, c[2] as i8 - c[1] as i8)
}
