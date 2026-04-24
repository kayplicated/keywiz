//! Content-addressed hashing for layout and keyboard snapshots.
//!
//! Consumers that want a stable content hash for their own types
//! implement [`Canonical`]. Two sessions ran "against the same
//! layout" iff their layouts produce identical canonical bytes —
//! so the conversion is where identity is decided. Choices like
//! "is the layout name part of identity?" (yes) or "is
//! last-modified timestamp part of identity?" (no) live in the
//! impl, not here.
//!
//! Hashing is SHA-256 over the canonical bytes, hex-encoded. Cheap
//! (microseconds for a ~2KB JSON) and stable across platforms.

use sha2::{Digest, Sha256};

use crate::snapshot::{KeyboardHash, LayoutHash};

/// Types that can be serialized into a deterministic canonical
/// form for content-hashing. Implementors must guarantee: same
/// semantic value → identical output bytes, across runs, machines,
/// and serialization of different instances of the same value.
///
/// The default path is `serde_json` with sorted map keys — that's
/// what [`canonical_json`] below gives you.
pub trait Canonical {
    /// Return the canonical bytes this value hashes over.
    fn to_canonical_bytes(&self) -> Vec<u8>;
}

/// Hash any [`Canonical`] value into a hex-encoded SHA-256 string.
pub fn hash_canonical<T: Canonical>(value: &T) -> String {
    let bytes = value.to_canonical_bytes();
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

/// Convenience: hash and wrap as [`LayoutHash`].
pub fn layout_hash<T: Canonical>(value: &T) -> LayoutHash {
    LayoutHash(hash_canonical(value))
}

/// Convenience: hash and wrap as [`KeyboardHash`].
pub fn keyboard_hash<T: Canonical>(value: &T) -> KeyboardHash {
    KeyboardHash(hash_canonical(value))
}

/// Serialize any `Serialize`-able value into a canonical JSON byte
/// stream: sorted object keys, no whitespace, numeric and string
/// escapes normalized. Suitable as the default `to_canonical_bytes`
/// implementation for most types.
///
/// Returns an error if serialization fails — practically only
/// reachable for non-serde-compatible types or types whose `Serialize`
/// impl panics.
pub fn canonical_json<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    // serde_json::to_value produces a `serde_json::Value`; we then
    // walk it re-emitting objects with keys sorted. That's the
    // standard "canonical JSON" approach.
    let raw = serde_json::to_value(value)?;
    let canonical = sort_keys(raw);
    serde_json::to_vec(&canonical)
}

fn sort_keys(value: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let sorted: serde_json::Map<String, Value> = entries
                .into_iter()
                .map(|(k, v)| (k, sort_keys(v)))
                .collect();
            Value::Object(sorted)
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(sort_keys).collect()),
        scalar => scalar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Fixture<'a> {
        b: i32,
        a: &'a str,
        c: Vec<i32>,
    }

    #[derive(Serialize)]
    struct FixtureReordered<'a> {
        c: Vec<i32>,
        a: &'a str,
        b: i32,
    }

    impl Canonical for Fixture<'_> {
        fn to_canonical_bytes(&self) -> Vec<u8> {
            canonical_json(self).expect("serialize")
        }
    }

    impl Canonical for FixtureReordered<'_> {
        fn to_canonical_bytes(&self) -> Vec<u8> {
            canonical_json(self).expect("serialize")
        }
    }

    #[test]
    fn hash_is_stable_across_field_declaration_order() {
        // Same semantic value, different struct-field declaration
        // order — must produce identical hashes. This is the whole
        // point of canonicalization.
        let a = Fixture {
            a: "hi",
            b: 42,
            c: vec![1, 2, 3],
        };
        let b = FixtureReordered {
            c: vec![1, 2, 3],
            a: "hi",
            b: 42,
        };
        assert_eq!(hash_canonical(&a), hash_canonical(&b));
    }

    #[test]
    fn hash_differs_for_different_values() {
        let a = Fixture {
            a: "hi",
            b: 42,
            c: vec![1, 2, 3],
        };
        let c = Fixture {
            a: "hi",
            b: 43, // different
            c: vec![1, 2, 3],
        };
        assert_ne!(hash_canonical(&a), hash_canonical(&c));
    }

    #[test]
    fn canonical_json_sorts_map_keys() {
        let v = serde_json::json!({ "z": 1, "a": 2, "m": 3 });
        let bytes = canonical_json(&v).unwrap();
        let s = std::str::from_utf8(&bytes).unwrap();
        // Keys appear in sorted order in the output.
        let pos_a = s.find("\"a\"").unwrap();
        let pos_m = s.find("\"m\"").unwrap();
        let pos_z = s.find("\"z\"").unwrap();
        assert!(pos_a < pos_m && pos_m < pos_z);
    }

    #[test]
    fn canonical_json_sorts_keys_recursively() {
        let v = serde_json::json!({
            "outer": { "z": 1, "a": 2 }
        });
        let bytes = canonical_json(&v).unwrap();
        let s = std::str::from_utf8(&bytes).unwrap();
        let pos_a = s.find("\"a\"").unwrap();
        let pos_z = s.find("\"z\"").unwrap();
        assert!(pos_a < pos_z, "nested keys should also be sorted: {s}");
    }

    #[test]
    fn hash_is_hex_and_stable_length() {
        let a = Fixture {
            a: "x",
            b: 1,
            c: vec![],
        };
        let h = hash_canonical(&a);
        assert_eq!(h.len(), 64, "SHA-256 hex is 64 chars");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
