//! Format-agnostic layout-diff computation.
//!
//! Walks the keyboard's key space; for each key, records the char
//! each layout binds there (if any). Emits only positions where
//! the two layouts disagree. Sort order is physical — row-major,
//! then left-to-right — so the output reads like the keyboard
//! itself rather than an alphabetized key-id list.

use std::collections::HashMap;

use drift_core::{Keyboard, Layout};

/// One disagreement between two layouts.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiffEntry {
    /// The key-id (as a plain string for downstream use).
    pub key_id: String,
    /// Char layout A places here, if any.
    pub a: Option<char>,
    /// Char layout B places here, if any.
    pub b: Option<char>,
}

/// Compute the per-key diff between `a` and `b` over `keyboard`.
/// Returns entries sorted by physical position (row, then x-left
/// to x-right). Only keys where the two layouts differ are
/// included.
pub fn diff(a: &Layout, b: &Layout, keyboard: &Keyboard) -> Vec<DiffEntry> {
    // Invert each layout's char → Key into key_id → char so we can
    // walk the keyboard's key space in order.
    let a_by_key = invert(a);
    let b_by_key = invert(b);

    let mut keys: Vec<_> = keyboard.keys.values().collect();
    keys.sort_by(|k, l| {
        // y-major (row), then x. Deterministic ties on id to keep
        // output stable regardless of HashMap iteration.
        k.y.partial_cmp(&l.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(k.x.partial_cmp(&l.x).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| k.id.as_str().cmp(l.id.as_str()))
    });

    let mut entries = Vec::new();
    for key in keys {
        let ac = a_by_key.get(key.id.as_str()).copied();
        let bc = b_by_key.get(key.id.as_str()).copied();
        if ac != bc {
            entries.push(DiffEntry {
                key_id: key.id.as_str().to_string(),
                a: ac,
                b: bc,
            });
        }
    }
    entries
}

fn invert(layout: &Layout) -> HashMap<&str, char> {
    let mut out = HashMap::with_capacity(layout.positions.len());
    for (ch, key) in &layout.positions {
        out.insert(key.id.as_str(), *ch);
    }
    out
}
