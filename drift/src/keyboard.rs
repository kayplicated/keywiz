//! Keyboard geometry loader. Reads keywiz-format keyboard JSON5
//! files and extracts per-key physical position + finger assignment.
//!
//! Scoring only uses the 30-key alpha core (main_k1..main_k30) —
//! number row, thumbs, and outer columns are not currently scored.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// All eight alpha-layer fingers. Matches keywiz `finger` strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Finger {
    LPinky,
    LRing,
    LMiddle,
    LIndex,
    RIndex,
    RMiddle,
    RRing,
    RPinky,
}

impl Finger {
    /// True if both fingers are on the same hand.
    pub fn same_hand(self, other: Finger) -> bool {
        use Finger::*;
        let lhs = matches!(self, LPinky | LRing | LMiddle | LIndex);
        let rhs = matches!(other, LPinky | LRing | LMiddle | LIndex);
        lhs == rhs
    }

    /// "Column distance" between two fingers on the same hand.
    /// 0 = same finger, 1 = adjacent, 4 = pinky-to-index.
    /// Returns `None` for cross-hand pairs.
    pub fn column_distance(self, other: Finger) -> Option<u8> {
        if !self.same_hand(other) {
            return None;
        }
        Some((self.index() as i8 - other.index() as i8).unsigned_abs())
    }

    /// 0..=3 index within hand (pinky=0, index=3).
    fn index(self) -> u8 {
        use Finger::*;
        match self {
            LPinky | RPinky => 0,
            LRing | RRing => 1,
            LMiddle | RMiddle => 2,
            LIndex | RIndex => 3,
        }
    }
}

impl std::fmt::Display for Finger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Finger::*;
        let s = match self {
            LPinky => "L-pinky",
            LRing => "L-ring",
            LMiddle => "L-middle",
            LIndex => "L-index",
            RIndex => "R-index",
            RMiddle => "R-middle",
            RRing => "R-ring",
            RPinky => "R-pinky",
        };
        f.write_str(s)
    }
}

/// A single physical key on the alpha core.
#[derive(Debug, Clone)]
pub struct Key {
    pub id: String,
    /// Column index. Negative = left hand (-5..-1), positive = right (1..5).
    /// Preserved for future column-distance heuristics; not currently scored.
    #[allow(dead_code)]
    pub col: i32,
    /// Row index. -1 = top, 0 = home, 1 = bottom, -2 = number.
    pub row: i32,
    /// Physical x in key-units.
    pub x: f64,
    /// Physical y in key-units. Lower y = physically closer to user
    /// (farther from the number row).
    pub y: f64,
    pub finger: Finger,
}

impl Key {
    /// True if this key is on the 30-key alpha core.
    /// Layout-resolution already filters by id prefix; kept here
    /// for explicit geometric checks in future code.
    #[allow(dead_code)]
    pub fn is_alpha_core(&self) -> bool {
        // Alpha core = rows -1, 0, 1 across cols -5..-1, 1..5.
        (self.row == -1 || self.row == 0 || self.row == 1)
            && (-5..=-1).contains(&self.col) || (1..=5).contains(&self.col)
    }
}

/// A keyboard's alpha-core geometry, keyed by key id.
#[derive(Debug, Clone)]
pub struct Keyboard {
    pub name: String,
    pub keys: HashMap<String, Key>,
}

/// Raw JSON structure matching keywiz's keyboard format.
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
}

impl Keyboard {
    /// Load a keyboard from its keywiz JSON5 definition.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading keyboard: {}", path.display()))?;
        let raw: RawKeyboard = json5::from_str(&text)
            .with_context(|| format!("parsing keyboard: {}", path.display()))?;

        let mut keys = HashMap::new();
        for block in raw.blocks {
            for rk in block.keys {
                // Skip keys on fingers outside the alpha-scoring set
                // (thumbs, etc.). Their absence from the map is
                // handled by layout-resolution filtering downstream.
                let Some(finger) = parse_finger(&rk.finger) else {
                    continue;
                };
                let key = Key {
                    id: rk.id.clone(),
                    col: rk.c,
                    row: rk.r,
                    x: rk.x,
                    y: rk.y,
                    finger,
                };
                keys.insert(rk.id, key);
            }
        }

        Ok(Keyboard { name: raw.name, keys })
    }

    /// Look up a key by id.
    pub fn key(&self, id: &str) -> Option<&Key> {
        self.keys.get(id)
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
