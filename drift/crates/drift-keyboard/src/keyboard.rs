//! Keyboard JSON5 reader.
//!
//! Reads the keywiz keyboard schema (`{ name, blocks: [{ keys: [...] }] }`)
//! into a [`Keyboard`]. Keys whose `finger` string doesn't name one
//! of the eight alpha fingers are skipped — thumb keys and similar
//! are out of scope for the scoring pipeline.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use drift_core::{Finger, FingerColumn, Key, KeyId, Keyboard, Row};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RawKeyboard {
    name: String,
    blocks: Vec<RawBlock>,
}

#[derive(Debug, Deserialize)]
struct RawBlock {
    #[serde(rename = "type")]
    _block_type: String,
    #[serde(rename = "cluster")]
    _cluster: Option<String>,
    keys: Vec<RawKey>,
}

#[derive(Debug, Deserialize)]
struct RawKey {
    id: String,
    r: i32,
    c: i32,
    x: f64,
    y: f64,
    finger: String,
    /// Optional sub-column override. Accepts `"primary"` or
    /// `"index_center"` (case-insensitive). When omitted, the
    /// loader infers from `c`: `|c| == 1` → index-center for
    /// index fingers, `Primary` otherwise.
    #[serde(default)]
    finger_column: Option<String>,
}

pub fn load(path: &Path) -> Result<Keyboard> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading keyboard: {}", path.display()))?;
    let raw: RawKeyboard = json5::from_str(&text)
        .with_context(|| format!("parsing keyboard: {}", path.display()))?;

    let mut keys: HashMap<KeyId, Key> = HashMap::new();
    for block in raw.blocks {
        for rk in block.keys {
            let Some(finger) = parse_finger(&rk.finger) else {
                continue;
            };
            let finger_column = resolve_finger_column(finger, rk.c, rk.finger_column.as_deref());
            let id = KeyId::new(rk.id);
            let key = Key {
                id: id.clone(),
                col: rk.c,
                row: row_from_raw(rk.r),
                x: rk.x,
                y: rk.y,
                finger,
                finger_column,
            };
            keys.insert(id, key);
        }
    }

    Ok(Keyboard {
        name: raw.name,
        keys,
    })
}

/// Decide the sub-column for a key. Explicit string overrides
/// whatever inference would say; otherwise we infer:
///
/// - Non-index fingers are always `Outer`.
/// - Index keys with `|col| == 1` are `Inner` (the inward-reach
///   column closer to the thumb, across the central gap on split
///   boards).
/// - Other index keys are `Outer` — the finger's home-index column.
///
/// The explicit form accepts either the canonical names
/// (`"outer"`, `"inner"`) or the older aliases (`"primary"`,
/// `"home"`, `"home_index"`, `"index_inner"`, `"index_center"`)
/// — the latter a historical synonym for the inner column.
fn resolve_finger_column(finger: Finger, c: i32, explicit: Option<&str>) -> FingerColumn {
    if let Some(s) = explicit {
        match s.to_ascii_lowercase().as_str() {
            "outer" | "primary" | "home" | "home_index" | "home-index" | "homeindex" => {
                return FingerColumn::Outer;
            }
            "inner" | "index_inner" | "index-inner" | "indexinner"
            | "index_center" | "index-center" | "indexcenter" => {
                return FingerColumn::Inner;
            }
            // Unknown string — fall through to inference rather
            // than erroring. A future version may tighten this.
            _ => {}
        }
    }
    match finger {
        Finger::LIndex | Finger::RIndex if c.unsigned_abs() == 1 => FingerColumn::Inner,
        _ => FingerColumn::Outer,
    }
}

/// Map the raw integer row from the JSON schema to a named [`Row`].
///
/// Schema convention: `-2` number row, `-1` top, `0` home, `1` bottom.
/// Anything else becomes `Row::Extra(n)` with `n = r - 2`.
fn row_from_raw(r: i32) -> Row {
    match r {
        -2 => Row::Number,
        -1 => Row::Top,
        0 => Row::Home,
        1 => Row::Bottom,
        other if other >= 2 => Row::Extra((other - 2) as u8),
        _ => Row::Home,
    }
}

fn parse_finger(s: &str) -> Option<Finger> {
    use Finger::*;
    Some(match s {
        "LPinky" => LPinky,
        "LRing" => LRing,
        "LMiddle" => LMiddle,
        "LIndex" => LIndex,
        "RIndex" => RIndex,
        "RMiddle" => RMiddle,
        "RRing" => RRing,
        "RPinky" => RPinky,
        _ => return None,
    })
}
