//! Asymmetric-forward exemption rule for col-stag keyboards.
//!
//! On column-staggered boards, neighboring fingers at different
//! physical y-positions are the natural resting shape, not a
//! scissor. This primitive lets analyzers decide whether a given
//! finger pair is "naturally splayed" and thus shouldn't count a
//! cross-row motion as a scissor.

use drift_core::Key;

/// Per-pair toggles for the asymmetric-forward rule. Each pair
/// can independently be exempted or not, and a minimum y-delta
/// gates the exemption so shallow staggers don't trigger it.
#[derive(Debug, Clone, Copy)]
pub struct AsymmetricRules {
    pub index_middle_forward_ok: bool,
    pub middle_ring_forward_ok: bool,
    pub ring_pinky_forward_ok: bool,
    /// Minimum absolute y-delta in key-units between the two keys
    /// for the exemption to apply. A value of 0 means any forward
    /// offset counts.
    pub forward_threshold: f64,
}

impl Default for AsymmetricRules {
    fn default() -> Self {
        Self {
            index_middle_forward_ok: true,
            middle_ring_forward_ok: true,
            ring_pinky_forward_ok: true,
            forward_threshold: 0.0,
        }
    }
}

/// Decide whether this bigram should be exempted from
/// scissor-style classification by the asymmetric-forward rule.
///
/// Returns `true` when:
/// - the two keys are on the same hand with adjacent fingers, and
/// - the outer (pinky-ward) finger is at a physically-forward
///   y-position relative to the inner finger, and
/// - the finger pair has its exemption enabled in `rules`, and
/// - the absolute y-delta meets the configured threshold.
pub fn is_forward_exempt(a: &Key, b: &Key, rules: &AsymmetricRules) -> bool {
    if !a.finger.same_hand(b.finger) {
        return false;
    }
    let Some(gap) = a.finger.column_distance(b.finger) else {
        return false;
    };
    if gap != 1 {
        return false;
    }

    // Identify outer (pinky-ward) and inner (thumb-ward). The
    // `column()` method returns 0=pinky..3=index, so the key with
    // the lower column index is the outer one.
    let (outer, inner) = if a.finger.column() < b.finger.column() {
        (a, b)
    } else {
        (b, a)
    };

    // "Forward" means outer is physically closer to the user, which
    // in keywiz coordinates means larger y. Change this mapping and
    // the whole rule inverts — intentional that the direction is
    // explicit rather than a config knob.
    if outer.y <= inner.y {
        return false;
    }
    if (outer.y - inner.y).abs() < rules.forward_threshold {
        return false;
    }

    // Match arms: `(outer_finger, inner_finger)`. Because `outer`
    // is pinky-ward (lower column) and `inner` is thumb-ward
    // (higher column), the pair for e.g. the index-middle adjacency
    // is `(Middle, Index)` — middle is outer, index is inner.
    // Earlier revisions of this code had these arms backwards and
    // were unreachable; the config field names still reference the
    // ergonomic pair (e.g. `index_middle_forward_ok`) rather than
    // the column ordering.
    use drift_core::Finger::*;
    match (outer.finger, inner.finger) {
        (LMiddle, LIndex) | (RMiddle, RIndex) => rules.index_middle_forward_ok,
        (LRing, LMiddle) | (RRing, RMiddle) => rules.middle_ring_forward_ok,
        (LPinky, LRing) | (RPinky, RRing) => rules.ring_pinky_forward_ok,
        _ => false,
    }
}
