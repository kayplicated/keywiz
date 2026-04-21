//! Layout loader. Reads keywiz-format layout JSON5 files and
//! resolves them against a [`Keyboard`] into a char -> key mapping.

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::keyboard::{Key, Keyboard};

/// A resolved layout: every key id from the mapping is joined with
/// its keyboard geometry. Only alpha-producing entries (`char`-type)
/// are retained — named keys (shift, tab, etc.) are skipped.
#[derive(Debug, Clone)]
pub struct Layout {
    pub name: String,
    /// Char -> position on the keyboard. Uppercase chars collapse to
    /// lowercase before scoring (matching corpus convention).
    pub positions: HashMap<char, Key>,
}

/// Raw layout file. keywiz format: `{ name, mappings: { key_id: binding } }`.
#[derive(Debug, Deserialize)]
struct RawLayout {
    name: String,
    mappings: IndexMap<String, Binding>,
}

/// A single key binding. `char` = produces character(s) (we score the
/// unshifted one). `named` = modifier/tab/enter/etc, skipped.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Binding {
    Char {
        #[serde(rename = "char")]
        chars: Vec<String>,
    },
    Named {
        #[serde(rename = "named")]
        _name: String,
    },
}

impl Layout {
    /// Load + resolve a layout against the given keyboard.
    pub fn load(path: &Path, keyboard: &Keyboard) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading layout: {}", path.display()))?;
        let raw: RawLayout = json5::from_str(&text)
            .with_context(|| format!("parsing layout: {}", path.display()))?;

        let mut positions = HashMap::new();
        for (key_id, binding) in raw.mappings {
            let chars = match binding {
                Binding::Char { chars } => chars,
                Binding::Named { .. } => continue,
            };

            let Some(first) = chars.first() else { continue };
            let Some(ch) = first.chars().next() else { continue };

            // Look up the physical key.
            let Some(key) = keyboard.key(&key_id) else {
                // Layout references a key not on this keyboard —
                // skip silently; not every keyboard has every slot.
                continue;
            };

            // Only score alpha-core keys (30-key main grid).
            if !is_alpha_core_id(&key_id) {
                continue;
            }

            positions.insert(ch.to_ascii_lowercase(), key.clone());
        }

        Ok(Layout { name: raw.name, positions })
    }

    /// Position of a character on this layout, if any.
    pub fn position(&self, ch: char) -> Option<&Key> {
        self.positions.get(&ch.to_ascii_lowercase())
    }
}

/// Whether a key id belongs to the 30-key alpha core (`main_k1..k30`).
fn is_alpha_core_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("main_k") else {
        return false;
    };
    let Ok(n) = rest.parse::<u32>() else {
        return false;
    };
    (1..=30).contains(&n)
}
