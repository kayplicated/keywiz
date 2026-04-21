//! Input-character translation across layouts.
//!
//! When the user's OS is sending characters from layout A but they're
//! training layout B, every keystroke needs to be re-interpreted:
//! character → physical id on layout A → character on layout B.
//!
//! `Translator::between` builds the char→char map from two layouts
//! that share ids. Layouts that don't share ids produce an empty
//! (identity) translator — cross-id-scheme training isn't meaningful.

use std::collections::HashMap;

use crate::mapping::{KeyMapping, Layout};

#[derive(Debug, Clone, Default)]
pub struct Translator {
    map: HashMap<char, char>,
}

impl Translator {
    pub fn identity() -> Self {
        Self::default()
    }

    /// Build a translator that maps characters from `from` to the
    /// character at the same id in `to`. Non-`Char` mappings and ids
    /// present in only one side are skipped.
    pub fn between(from: &Layout, to: &Layout) -> Self {
        let mut map = HashMap::new();
        for (id, from_map) in &from.mappings {
            let Some(to_map) = to.mappings.get(id) else {
                continue;
            };
            if let (
                KeyMapping::Char {
                    lower: fl,
                    upper: fu,
                },
                KeyMapping::Char {
                    lower: tl,
                    upper: tu,
                },
            ) = (from_map, to_map)
            {
                map.insert(*fl, *tl);
                map.insert(*fu, *tu);
            }
        }
        Translator { map }
    }

    pub fn translate(&self, ch: char) -> char {
        self.map.get(&ch).copied().unwrap_or(ch)
    }
}

/// Build a translator from the named input layout to the active
/// target layout. Returns identity when `from_layout` is `None` or
/// when the from-layout fails to load.
pub fn build(target: &Layout, from_layout: Option<&str>) -> Translator {
    let Some(from_name) = from_layout else {
        return Translator::identity();
    };
    let from_path = std::path::Path::new("layouts").join(format!("{from_name}.json"));
    let Ok(from_layout_data) = crate::mapping::loader::load(&from_path) else {
        return Translator::identity();
    };
    Translator::between(&from_layout_data, target)
}
