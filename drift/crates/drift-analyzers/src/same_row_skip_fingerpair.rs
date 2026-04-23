//! Same-row non-adjacent motion with per-finger-pair weights.
//!
//! A companion to [`crate::same_row_skip`], which uses a single flat
//! weight. This analyzer instead keys rewards by the specific
//! (outer-finger, inner-finger, direction) triple, so a user whose
//! kinesthetic map distinguishes e.g. `ring→index` (good) from
//! `pinky→ring` (slightly awkward) can encode that without compromise.
//!
//! "Same-row non-adjacent" means: both keys on the same hand, same
//! row, with at least one alpha column (on the same row) physically
//! between them. In other words, the two keys' column indices differ
//! by ≥ 2. This differs from finger-based adjacency — a bigram like
//! `of` with ring→outer-index skips middle but is finger-adjacent
//! from the perspective of `Finger::column_distance`.
//!
//! The analyzer can't know which weights make sense for your hands.
//! All weights default to 0 — the analyzer is a measurement tool
//! enabled in presets that opt in. Drifter's preset ships values
//! matching Kay's subjective feel; neutral/extension/oxey_mimic
//! leave them at 0.
//!
//! Keys follow the pattern `<direction>_<outer>_<inner>` where
//! outer is the pinky-ward finger and inner is the thumb-ward
//! finger. For example:
//!
//! ```toml
//! [analyzers.same_row_skip_fingerpair]
//! inward_ring_index = 1.5       # e.g. R-hand `of` on Drifter
//! outward_index_ring = 0.8      # same pair, other direction
//! inward_pinky_index_inner = 1.5  # e.g. R-hand `yk` on Drifter
//! ```
//!
//! Index finger sub-columns matter because reaching the inner
//! column (across the split gap) is a physically distinct motion
//! from reaching the outer column. Entries ending in `_index` refer
//! to the outer (home-index) column; entries ending in `_index_inner`
//! refer to the inner column. Non-index fingers have only one
//! column so no suffix is needed.

use std::collections::HashMap;

use anyhow::Result;
use drift_analyzer::{f64_or, Analyzer, AnalyzerEntry, ConfigValue, Registry};
use drift_core::{Finger, FingerColumn, Hit, Scope, Window};

pub fn register(registry: &mut Registry) {
    registry.register(AnalyzerEntry {
        name: "same_row_skip_fingerpair",
        build: |cfg| Ok(Box::new(SameRowSkipFingerpair::from_config(cfg)?)),
    });
}

/// A direction for a same-hand roll motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Direction {
    /// Outer finger to inner finger — pinky-ward to thumb-ward.
    Inward,
    /// Inner finger to outer finger.
    Outward,
}

/// A specific finger+sub-column key for the weight lookup table.
/// Non-index fingers always use `FingerColumn::Outer`; index can be
/// either.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FingerSlot {
    kind: FingerKind,
    sub: FingerColumn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FingerKind {
    Pinky,
    Ring,
    Middle,
    Index,
}

impl FingerKind {
    fn from(f: Finger) -> Self {
        use Finger::*;
        match f {
            LPinky | RPinky => FingerKind::Pinky,
            LRing | RRing => FingerKind::Ring,
            LMiddle | RMiddle => FingerKind::Middle,
            LIndex | RIndex => FingerKind::Index,
            // Non-exhaustive future variants (e.g. thumb-as-alpha).
            // Conservatively treat unknown fingers as pinky — that
            // yields zero-weight lookups in this analyzer's table
            // rather than panicking.
            _ => FingerKind::Pinky,
        }
    }
}

/// Weight-table key: (direction, outer, inner).
type WeightKey = (Direction, FingerSlot, FingerSlot);

pub struct SameRowSkipFingerpair {
    /// Map of finger-pair-and-direction triples to their weights.
    /// Entries not present in the map contribute nothing.
    weights: HashMap<WeightKey, f64>,
}

impl SameRowSkipFingerpair {
    pub fn from_config(cfg: Option<&dyn ConfigValue>) -> Result<Self> {
        let mut weights = HashMap::new();
        // The full set of valid same-row non-adjacent finger pairs.
        // Each entry in this list corresponds to one config key of
        // the form `<direction>_<outer>_<inner>`.
        //
        // Non-index fingers have only `FingerColumn::Outer`; index
        // fingers have both `Outer` and `Inner`. "Non-adjacent"
        // means the key distance is ≥2 physical columns apart, so
        // some adjacent-finger pairs (e.g. middle→index-outer) are
        // excluded here — they're rolls, not skips.
        //
        // Inward = outer-to-inner. Outward = inner-to-outer.
        let pairs = [
            // Pinky to ring is finger-adjacent; excluded.
            // Pinky skipping one finger reaches middle:
            ("pinky_middle", Self::slot_pinky(), Self::slot_middle()),
            // Pinky skipping two fingers reaches index-outer:
            ("pinky_index", Self::slot_pinky(), Self::slot_index_outer()),
            // Pinky skipping three fingers reaches index-inner:
            ("pinky_index_inner", Self::slot_pinky(), Self::slot_index_inner()),
            // Ring skipping middle reaches index-outer:
            ("ring_index", Self::slot_ring(), Self::slot_index_outer()),
            // Ring skipping middle and index-outer reaches index-inner:
            ("ring_index_inner", Self::slot_ring(), Self::slot_index_inner()),
            // Middle to index-inner skips index-outer:
            ("middle_index_inner", Self::slot_middle(), Self::slot_index_inner()),
        ];
        for (suffix, outer, inner) in &pairs {
            for (direction, prefix) in
                [(Direction::Inward, "inward"), (Direction::Outward, "outward")]
            {
                let key = format!("{prefix}_{suffix}");
                let w = f64_or(cfg, &key, 0.0);
                if w != 0.0 {
                    weights.insert((direction, *outer, *inner), w);
                }
            }
        }
        Ok(Self { weights })
    }

    fn slot_pinky() -> FingerSlot {
        FingerSlot { kind: FingerKind::Pinky, sub: FingerColumn::Outer }
    }
    fn slot_ring() -> FingerSlot {
        FingerSlot { kind: FingerKind::Ring, sub: FingerColumn::Outer }
    }
    fn slot_middle() -> FingerSlot {
        FingerSlot { kind: FingerKind::Middle, sub: FingerColumn::Outer }
    }
    fn slot_index_outer() -> FingerSlot {
        FingerSlot { kind: FingerKind::Index, sub: FingerColumn::Outer }
    }
    fn slot_index_inner() -> FingerSlot {
        FingerSlot { kind: FingerKind::Index, sub: FingerColumn::Inner }
    }
}

fn slot_of(finger: Finger, col: FingerColumn) -> FingerSlot {
    FingerSlot {
        kind: FingerKind::from(finger),
        sub: col,
    }
}

impl Analyzer for SameRowSkipFingerpair {
    fn name(&self) -> &'static str {
        "same_row_skip_fingerpair"
    }

    fn scope(&self) -> Scope {
        Scope::Bigram
    }

    fn evaluate(&self, window: &Window) -> Vec<Hit> {
        let a = window.keys[0];
        let b = window.keys[1];

        // Same hand, same row, different fingers.
        if !a.finger.same_hand(b.finger) || a.finger == b.finger || a.row != b.row {
            return Vec::new();
        }
        // Non-adjacent by physical column distance (≥ 2).
        let col_distance = (a.col - b.col).unsigned_abs();
        if col_distance < 2 {
            return Vec::new();
        }

        // Identify outer (pinky-ward, lower finger column) and inner.
        let (outer_key, inner_key, direction) = if a.finger.column() < b.finger.column() {
            // a is outer, b is inner → outer-to-inner is inward.
            (a, b, Direction::Inward)
        } else if a.finger.column() > b.finger.column() {
            (b, a, Direction::Outward)
        } else {
            // Same finger (shouldn't happen — we already excluded
            // `a.finger == b.finger` above), but handle gracefully.
            return Vec::new();
        };

        let outer_slot = slot_of(outer_key.finger, outer_key.finger_column);
        let inner_slot = slot_of(inner_key.finger, inner_key.finger_column);

        let weight = self
            .weights
            .get(&(direction, outer_slot, inner_slot))
            .copied()
            .unwrap_or(0.0);
        if weight == 0.0 {
            return Vec::new();
        }

        let dir_label = match direction {
            Direction::Inward => "in",
            Direction::Outward => "out",
        };
        vec![Hit {
            category: "same_row_skip_fingerpair",
            label: format!("{dir_label} {}{}", window.chars[0], window.chars[1]),
            cost: window.freq * weight,
        }]
    }
}
