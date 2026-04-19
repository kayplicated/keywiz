//! Input-character translation by physical id.
//!
//! When the user's terminal sends characters from one layout but they're
//! practicing another, each character has to be re-mapped: the character
//! they typed → the id on the *input* keyboard that produces it → what
//! the *target* layout says that id does.
//!
//! Identity is the physical id. Cross-keyboard training works when the
//! two keyboards use matching id schemes (the suggested convention);
//! pairs of keyboards that don't share ids simply don't translate
//! meaningfully.

use std::collections::HashMap;

use crate::configreader::keyboard as kb_reader;
use crate::configreader::layout as layout_reader;
use crate::grid::layout::KeyMapping;
use crate::grid::Grid;

/// Translates characters from input-keyboard representation to the
/// target layout's representation.
#[derive(Debug, Clone, Default)]
pub struct Translator {
    map: HashMap<char, char>,
}

impl Translator {
    pub fn identity() -> Self {
        Self::default()
    }

    /// Build a translator that maps characters produced by `from` to
    /// the equivalent characters produced by `to`, id by id. Ids
    /// present in one grid but not the other are skipped. Non-[`Char`]
    /// mappings are skipped (named keys don't translate into typed
    /// characters).
    pub fn between(from: &Grid, to: &Grid) -> Self {
        let target: HashMap<&str, &KeyMapping> = to
            .buttons
            .iter()
            .filter_map(|b| b.mapping.as_ref().map(|m| (b.id.as_str(), m)))
            .collect();

        let mut map = HashMap::new();
        for btn in &from.buttons {
            let Some(from_map) = &btn.mapping else {
                continue;
            };
            let Some(to_map) = target.get(btn.id.as_str()) else {
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

    pub fn is_identity(&self) -> bool {
        self.map.is_empty()
    }
}

/// Build a translator from the input keyboard (running `from_layout`)
/// to the active `target` grid. Returns identity when `from_layout` is
/// `None`, or when anything on the from-side fails to load.
pub fn build(target: &Grid, from_layout: Option<&str>) -> Translator {
    let Some(from_name) = from_layout else {
        return Translator::identity();
    };
    let from_path = std::path::Path::new("layouts").join(format!("{from_name}.json"));
    let Ok(from_layout_data) = layout_reader::load(&from_path) else {
        // From-layout failed to load; silently fall back to identity.
        // `--from` is pre-validated at startup (main.rs), so a runtime
        // failure here means the on-disk file changed while running —
        // rare, and crashing the typing session over it is worse than
        // a silent identity.
        return Translator::identity();
    };
    let kb_path = std::path::Path::new("keyboards").join(format!("{}.json", target.keyboard_name));
    let Ok(keyboard) = kb_reader::load(&kb_path) else {
        return Translator::identity();
    };
    let from_grid = Grid::compose(&keyboard, &from_layout_data);
    Translator::between(&from_grid, target)
}
