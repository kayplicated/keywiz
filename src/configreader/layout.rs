//! JSON reader for character layouts.
//!
//! Schema:
//! ```json
//! {
//!   "name": "gallium-v2",
//!   "short": "Gallium v2",
//!   "mappings": {
//!     "main_k11": { "char": ["n", "N"] },
//!     "mods_shift_left": { "named": "shift" },
//!     "left_thumb_k3": { "char": [" ", " "] }
//!   }
//! }
//! ```
//!
//! Each mapping entry is either `{ "char": ["<lower>", "<upper>"] }` or
//! `{ "named": "<name>" }`. Missing ids are allowed — the resulting
//! layout is a partial function and keys not covered are simply dead.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::grid::layout::{KeyMapping, Layout};

/// On-disk mapping entry.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MappingRecord {
    /// Typed character: `{ "char": ["n", "N"] }`
    Char(Vec<String>),
    /// Non-character action: `{ "named": "shift" }`
    Named(String),
}

/// On-disk layout record.
#[derive(Debug, Deserialize)]
struct LayoutRecord {
    name: String,
    short: String,
    mappings: HashMap<String, MappingRecord>,
}

/// Load a layout JSON file.
pub fn load(path: &Path) -> Result<Layout, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let record: LayoutRecord =
        serde_json::from_str(&content).map_err(|e| format!("parsing {}: {e}", path.display()))?;

    let mut mappings = HashMap::with_capacity(record.mappings.len());
    for (id, m) in record.mappings {
        let mapping = match m {
            MappingRecord::Char(chars) => {
                if chars.len() != 2 {
                    return Err(format!(
                        "parsing {}: mapping for {id}: char needs exactly 2 entries (lower, upper)",
                        path.display()
                    ));
                }
                let lower = single_char(&chars[0]).ok_or_else(|| {
                    format!(
                        "parsing {}: mapping for {id}: char[0] must be a single character",
                        path.display()
                    )
                })?;
                let upper = single_char(&chars[1]).ok_or_else(|| {
                    format!(
                        "parsing {}: mapping for {id}: char[1] must be a single character",
                        path.display()
                    )
                })?;
                KeyMapping::Char { lower, upper }
            }
            MappingRecord::Named(name) => KeyMapping::Named { name },
        };
        mappings.insert(id, mapping);
    }

    Ok(Layout {
        name: record.name,
        short: record.short,
        mappings,
    })
}

fn single_char(s: &str) -> Option<char> {
    let mut chars = s.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    Some(first)
}
