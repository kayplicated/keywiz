//! Glue between keywiz's native types and the keywiz-stats crate.
//!
//! `keywiz_stats::Canonical` is a foreign trait (in keywiz-stats);
//! `mapping::Layout` / `keyboard::Keyboard` are domestic types. To
//! hash them via the stats crate we'd need to `impl Canonical for
//! Layout` — but the orphan rule forbids implementing a foreign
//! trait on a local type from a *different* crate when the type
//! comes from yet another local module… actually, both `Canonical`
//! and `Layout` are reachable from `keywiz` the binary, so the
//! orphan rule wouldn't bite here. The reason we use newtypes
//! anyway is scoping: the canonical form is an *opinion* about
//! what identity means for a layout, and that opinion belongs at
//! the seam between keywiz and stats, not inside `mapping::Layout`.
//!
//! Two newtypes: `LayoutDigest` and `KeyboardDigest`. Each wraps a
//! borrow of the source type and implements `Canonical` by
//! serializing a deterministic fingerprint (sorted map entries,
//! tagged enum variants, stable field order).

use keywiz_stats::{Canonical, KeyboardSnapshot, LayoutSnapshot, canonical_json};
use serde::Serialize;

use crate::keyboard::Keyboard;
use crate::mapping::{KeyMapping, Layout};

/// Canonical identity of a layout: its short name plus every
/// (key_id → mapping) binding. Geometry is *not* part of layout
/// identity — that's the keyboard's job. Two sessions share a
/// layout hash iff their layouts produce identical characters on
/// identical key ids.
pub struct LayoutDigest<'a>(pub &'a Layout);

impl Canonical for LayoutDigest<'_> {
    fn to_canonical_bytes(&self) -> Vec<u8> {
        canonical_json(&Fingerprint::from(self.0)).expect("layout canonical_json")
    }
}

/// Canonical identity of a keyboard: name plus every physical key
/// (id, finger, grid position, geometric position, cluster). A
/// keyboard's identity is its full geometry — moving a single key
/// by 0.1 units produces a different hash, which is exactly what
/// "keyboard + layout are one unit for performance" demands.
pub struct KeyboardDigest<'a>(pub &'a dyn Keyboard);

impl Canonical for KeyboardDigest<'_> {
    fn to_canonical_bytes(&self) -> Vec<u8> {
        canonical_json(&KeyboardFingerprint::from(self.0))
            .expect("keyboard canonical_json")
    }
}

// ---- serializable fingerprints ----
//
// These exist so `canonical_json` can do the sorted-keys work.
// They're intentionally flat and boring: no borrow tricks, no
// Option<String> fields that might serialize differently across
// releases. If a field here changes, every existing stat file
// gets new hashes — handle it the way drift handles corpus
// evolution: ship a migration.

#[derive(Serialize)]
struct Fingerprint<'a> {
    short: &'a str,
    /// Sorted by key_id so HashMap iteration order doesn't leak.
    mappings: Vec<(&'a str, MappingFingerprint<'a>)>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum MappingFingerprint<'a> {
    Char { lower: char, upper: char },
    Named { name: &'a str },
}

impl<'a> From<&'a Layout> for Fingerprint<'a> {
    fn from(layout: &'a Layout) -> Self {
        let mut mappings: Vec<(&'a str, MappingFingerprint<'a>)> = layout
            .mappings
            .iter()
            .map(|(id, m)| (id.as_str(), MappingFingerprint::from(m)))
            .collect();
        mappings.sort_by(|a, b| a.0.cmp(b.0));
        Self {
            short: &layout.short,
            mappings,
        }
    }
}

impl<'a> From<&'a KeyMapping> for MappingFingerprint<'a> {
    fn from(m: &'a KeyMapping) -> Self {
        match m {
            KeyMapping::Char { lower, upper } => MappingFingerprint::Char {
                lower: *lower,
                upper: *upper,
            },
            KeyMapping::Named { name } => MappingFingerprint::Named { name },
        }
    }
}

#[derive(Serialize)]
struct KeyboardFingerprint<'a> {
    name: &'a str,
    /// Sorted by id.
    keys: Vec<KeyFingerprint<'a>>,
}

#[derive(Serialize)]
struct KeyFingerprint<'a> {
    id: &'a str,
    r: i32,
    c: i32,
    // Geometry is stored as f32 in keywiz but serialized as strings
    // to dodge NaN / -0.0 / trailing-zero representation quirks in
    // serde_json. Hash must be bit-stable across machines.
    x: String,
    y: String,
    width: String,
    height: String,
    rotation: String,
    cluster: String,
    finger: String,
}

impl<'a> From<&'a dyn Keyboard> for KeyboardFingerprint<'a> {
    fn from(kb: &'a dyn Keyboard) -> Self {
        let mut keys: Vec<KeyFingerprint<'a>> = kb
            .keys()
            .map(|k| KeyFingerprint {
                id: k.id.as_str(),
                r: k.r,
                c: k.c,
                x: format_f32(k.x),
                y: format_f32(k.y),
                width: format_f32(k.width),
                height: format_f32(k.height),
                rotation: format_f32(k.rotation),
                cluster: format!("{:?}", k.cluster),
                finger: format!("{:?}", k.finger),
            })
            .collect();
        keys.sort_by(|a, b| a.id.cmp(b.id));
        Self { name: kb.name(), keys }
    }
}

/// Stable formatter for `f32` used in the hash. Six decimals is
/// generous for key-width-unit coordinates and keeps the fraction
/// consistent between 1.0 and 1.000000 at the representation level.
fn format_f32(v: f32) -> String {
    // `{:.6}` gives "1.000000" not "1" — critical for stability.
    // NaN shouldn't appear in loaded keyboards, but if it ever did,
    // format as "NaN" so the hash is still deterministic.
    if v.is_nan() {
        "NaN".to_string()
    } else {
        format!("{v:.6}")
    }
}

// ---- snapshot builders ----
//
// Convenience: given a live layout/keyboard and the current wall-
// clock time, produce a ready-to-pass LayoutSnapshot / KeyboardSnapshot
// the engine hands straight to Stats::begin_session.

/// Build a `LayoutSnapshot` from a live layout + display name.
/// `first_seen_ms` should be the current wall-clock time; the
/// store will discard it if the hash already exists.
pub fn layout_snapshot(layout: &Layout, display_name: &str, now_ms: i64) -> LayoutSnapshot {
    let digest = LayoutDigest(layout);
    let canonical_bytes = digest.to_canonical_bytes();
    LayoutSnapshot {
        hash: keywiz_stats::layout_hash(&digest),
        name: display_name.to_string(),
        canonical_json: String::from_utf8(canonical_bytes)
            .expect("canonical_json is always valid utf-8"),
        first_seen_ms: now_ms,
    }
}

/// Build a `KeyboardSnapshot` from a live keyboard + display name.
pub fn keyboard_snapshot(
    keyboard: &dyn Keyboard,
    display_name: &str,
    now_ms: i64,
) -> KeyboardSnapshot {
    let digest = KeyboardDigest(keyboard);
    let canonical_bytes = digest.to_canonical_bytes();
    KeyboardSnapshot {
        hash: keywiz_stats::keyboard_hash(&digest),
        name: display_name.to_string(),
        canonical_json: String::from_utf8(canonical_bytes)
            .expect("canonical_json is always valid utf-8"),
        first_seen_ms: now_ms,
    }
}
