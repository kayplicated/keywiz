//! Input-character translation by physical position.
//!
//! When the user's keyboard sends characters from one layout but they're
//! training another (e.g. SSH'd in from a QWERTY machine while practicing
//! Gallium on the remote host), every keypress needs to be translated.
//! The translation is defined by **physical position**: pressing the key
//! at position X on the input keyboard counts as whatever the target
//! layout has at position X.
//!
//! [`Translator`] carries the char→char map. Callers always see a single
//! type — [`Translator::identity`] is the "no translation" case, so code
//! doesn't branch on `Option`.

use std::collections::HashMap;

use crate::grid::{self, Grid};

/// Translates characters from input-keyboard representation to the
/// target layout's representation.
#[derive(Debug, Clone, Default)]
pub struct Translator {
    map: HashMap<char, char>,
}

impl Translator {
    /// No translation — input characters pass through unchanged. Used
    /// when the physical keyboard already produces the target layout.
    pub fn identity() -> Self {
        Self::default()
    }

    /// Build a translator that maps input characters produced by `from`
    /// to the equivalent characters produced by `to`, keycode by keycode.
    /// Keycodes present in one grid but not the other are skipped.
    pub fn between(from: &Grid, to: &Grid) -> Self {
        let target: HashMap<&str, &super::grid::KeyMapping> = to
            .buttons
            .iter()
            .filter_map(|b| b.mapping.as_ref().map(|m| (b.code.as_str(), m)))
            .collect();

        let mut map = HashMap::new();
        for btn in &from.buttons {
            let Some(from_map) = &btn.mapping else {
                continue;
            };
            let Some(to_map) = target.get(btn.code.as_str()) else {
                continue;
            };
            map.insert(from_map.lower, to_map.lower);
            map.insert(from_map.upper, to_map.upper);
        }
        Translator { map }
    }

    /// Translate a single character. Characters not in the map pass
    /// through unchanged — this matches the intuition that a key the
    /// translator doesn't know about just types what it types.
    pub fn translate(&self, ch: char) -> char {
        self.map.get(&ch).copied().unwrap_or(ch)
    }

    /// Is this an identity translator? Callers that want to display
    /// "training X while typing Y" UI can check this.
    pub fn is_identity(&self) -> bool {
        self.map.is_empty()
    }
}

/// Build a translator from the input keyboard (running `from_layout`) to
/// the active `target` grid. `from_layout` names a layout in `layouts/`
/// (e.g. `"qwerty"`). Returns [`Translator::identity`] when `from_layout`
/// is `None`.
///
/// Called at startup and again whenever the user cycles keyboard or
/// layout so the translator tracks the current target.
pub fn build(target: &Grid, from_layout: Option<&str>) -> Translator {
    let Some(from_name) = from_layout else {
        return Translator::identity();
    };
    // Compose the from-layout against the same physical keyboard so
    // positional semantics match.
    let from_path = std::path::Path::new("layouts").join(format!("{from_name}.json"));
    let from_layout_data = match grid::Layout::load(&from_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("keywiz: --from {from_name}: {e}");
            return Translator::identity();
        }
    };
    let kb_path = std::path::Path::new("keyboards").join(format!("{}.json", target.keyboard_name));
    let keyboard = grid::Keyboard::load(&kb_path).unwrap_or_else(|_| {
        // Kanata-derived grids don't have a matching keyboard file — fall
        // back to us_intl so translation still works.
        grid::Keyboard::load(std::path::Path::new("keyboards/us_intl.json"))
            .expect("us_intl.json should always be present")
    });
    let from_grid = Grid::compose(&keyboard, &from_layout_data);
    Translator::between(&from_grid, target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::{Keyboard, Layout};
    use std::path::Path;

    fn load_us_intl() -> Keyboard {
        Keyboard::load(Path::new("keyboards/us_intl.json")).expect("us_intl")
    }

    fn load_layout(name: &str) -> Layout {
        Layout::load(&Path::new("layouts").join(format!("{name}.json"))).expect(name)
    }

    #[test]
    fn identity_passes_characters_through() {
        let t = Translator::identity();
        assert_eq!(t.translate('a'), 'a');
        assert_eq!(t.translate('!'), '!');
        assert!(t.is_identity());
    }

    #[test]
    fn between_qwerty_and_colemak_remaps_by_position() {
        let kb = load_us_intl();
        let qwerty = Grid::compose(&kb, &load_layout("qwerty"));
        let colemak = Grid::compose(&kb, &load_layout("colemak"));
        let t = Translator::between(&qwerty, &colemak);

        // Pressing QWERTY 'e' (physical KEY_E) should map to Colemak's 'f'.
        assert_eq!(t.translate('e'), 'f');
        // Shifted variant: QWERTY 'E' → Colemak 'F'.
        assert_eq!(t.translate('E'), 'F');
        // Home row pinky: QWERTY 'a' → Colemak 'a' (unchanged).
        assert_eq!(t.translate('a'), 'a');
        // Not an identity overall.
        assert!(!t.is_identity());
    }

    #[test]
    fn between_identical_grids_is_effectively_identity() {
        let kb = load_us_intl();
        let g1 = Grid::compose(&kb, &load_layout("qwerty"));
        let g2 = Grid::compose(&kb, &load_layout("qwerty"));
        let t = Translator::between(&g1, &g2);
        assert_eq!(t.translate('q'), 'q');
        assert_eq!(t.translate('z'), 'z');
    }

    #[test]
    fn unknown_characters_pass_through() {
        let kb = load_us_intl();
        let qwerty = Grid::compose(&kb, &load_layout("qwerty"));
        let colemak = Grid::compose(&kb, &load_layout("colemak"));
        let t = Translator::between(&qwerty, &colemak);
        // Space isn't in any mapping — should pass through.
        assert_eq!(t.translate(' '), ' ');
    }
}
