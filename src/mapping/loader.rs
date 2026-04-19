//! JSON5 reader for layout files.
//!
//! Schema:
//! ```json5
//! {
//!   name: "gallium-v2",
//!   short: "Gallium v2",
//!   mappings: {
//!     main_k11: { char: ["n", "N"] },
//!     mods_shift_left: { named: "shift" },
//!     left_thumb_k3: { char: [" ", " "] },
//!   }
//! }
//! ```

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::mapping::{KeyMapping, Layout};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MappingRecord {
    Char(Vec<String>),
    Named(String),
}

#[derive(Debug, Deserialize)]
struct LayoutRecord {
    name: String,
    short: String,
    mappings: HashMap<String, MappingRecord>,
}

pub fn load(path: &Path) -> Result<Layout, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let record: LayoutRecord =
        json5::from_str(&content).map_err(|e| format!("parsing {}: {e}", path.display()))?;

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
