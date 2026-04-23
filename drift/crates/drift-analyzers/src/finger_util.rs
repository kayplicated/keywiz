//! Finger-name parsing helper.
//!
//! Config files refer to fingers by strings like `"l_pinky"` or
//! `"r_index"`. Analyzers that accept finger lists (e.g. the
//! redirect anchor set) route through this to parse them.

use drift_core::Finger;

/// Parse a finger name from config — case-insensitive, accepts
/// both `l_pinky` and `lpinky` / `LPinky` forms. Returns `None`
/// for unknown names.
pub fn parse_finger_name(name: &str) -> Option<Finger> {
    use Finger::*;
    let normalized: String = name
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_' && *c != '-')
        .flat_map(|c| c.to_lowercase())
        .collect();
    Some(match normalized.as_str() {
        "lpinky" => LPinky,
        "lring" => LRing,
        "lmiddle" => LMiddle,
        "lindex" => LIndex,
        "rindex" => RIndex,
        "rmiddle" => RMiddle,
        "rring" => RRing,
        "rpinky" => RPinky,
        _ => return None,
    })
}
