//! Layout JSON5 reader.
//!
//! Reads the keywiz layout schema (`{ name, mappings: { key_id: binding } }`),
//! resolves each binding against the given [`Keyboard`], and returns
//! a [`Layout`] with char → key positions. Non-alpha bindings (tab,
//! shift, etc.) and unmapped key ids are skipped silently — not
//! every keyboard has every slot.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use drift_core::{KeyId, Keyboard, Layout};
use indexmap::IndexMap;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RawLayout {
    name: String,
    mappings: IndexMap<String, Binding>,
}

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

pub fn load(path: &Path, keyboard: &Keyboard) -> Result<Layout> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading layout: {}", path.display()))?;
    let raw: RawLayout = json5::from_str(&text)
        .with_context(|| format!("parsing layout: {}", path.display()))?;

    let mut positions = HashMap::new();
    for (raw_id, binding) in raw.mappings {
        let chars = match binding {
            Binding::Char { chars } => chars,
            Binding::Named { .. } => continue,
        };
        let Some(first) = chars.first() else { continue };
        let Some(ch) = first.chars().next() else { continue };

        if !is_alpha_core_id(&raw_id) {
            continue;
        }

        let key_id = KeyId::new(raw_id);
        let Some(key) = keyboard.key(&key_id) else {
            continue;
        };
        positions.insert(ch.to_ascii_lowercase(), key.clone());
    }

    Ok(Layout {
        name: raw.name,
        positions,
    })
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
