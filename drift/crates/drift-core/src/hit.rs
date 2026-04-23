//! The `Hit` record — one contribution from one analyzer.
//!
//! Every analyzer emits zero or more hits per window it evaluates.
//! Hits accumulate additively into the total score. The category
//! is a stable identifier used for report rollups; the label is a
//! per-hit human-readable description.

/// One signed contribution to a layout's total score.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Hit {
    /// Stable category identifier. Multiple analyzers may emit into
    /// the same category; reports aggregate by this field.
    ///
    /// Currently `&'static str`. If dynamic categories become
    /// necessary (e.g. per-finger emit), this can be upgraded to
    /// `Cow<'static, str>` as a source-compatible change.
    pub category: &'static str,

    /// Human-readable label for this specific hit. Free-form; used
    /// in per-hit report lines.
    pub label: String,

    /// Signed contribution. Positive = reward, negative = penalty.
    pub cost: f64,
}
