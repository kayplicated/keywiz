//! Resolve a parsed [`DofDocument`] into a [`drift_core::Layout`].
//!
//! The `.dof` format carries three rows of space-separated chars;
//! drift speaks char→`Key`. We walk the rows and look up each
//! character's canonical `main_kN` id (per `docs/id-convention.md`)
//! against the provided keyboard. Keys the keyboard doesn't declare
//! are silently dropped — same convention as the keywiz layout
//! loader.
//!
//! Canonical mapping:
//!
//! ```text
//! row 0, cols 0..10  →  main_k1..main_k10   (top)
//! row 1, cols 0..10  →  main_k11..main_k20  (home)
//! row 2, cols 0..10  →  main_k21..main_k30  (bottom)
//! ```
//!
//! ANSI special case: the home row in a `.dof` ansi layout can
//! carry an 11th char — that's the `'` slot. Maps to `main_k48`
//! (right outer-pinky, home row) per the id convention.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use drift_core::{KeyId, Keyboard, Layout};

use crate::parse::DofDocument;

/// Resolve a parsed `.dof` document against `keyboard`.
pub fn resolve(doc: &DofDocument, keyboard: &Keyboard) -> Result<Layout> {
    if doc.rows.len() != 3 {
        return Err(anyhow!(
            "expected 3 rows, got {} (parser should have caught this)",
            doc.rows.len()
        ));
    }

    let mut positions = HashMap::new();
    for (row_idx, row) in doc.rows.iter().enumerate() {
        for (col_idx, token) in row.iter().enumerate() {
            let Some(key_id) = id_for(row_idx, col_idx) else {
                continue;
            };
            let Some(ch) = token.chars().next() else { continue };
            let Some(key) = keyboard.key(&KeyId::new(key_id)) else {
                continue;
            };
            positions.insert(ch.to_ascii_lowercase(), key.clone());
        }
    }

    Ok(Layout {
        name: doc.name.clone(),
        positions,
    })
}

/// Canonical `main_kN` id for a `.dof` (row, col) coordinate.
///
/// Returns `None` for positions outside the supported ranges
/// (more than 11 chars per row, row ≥ 3). The 11th char on the
/// home row maps to `main_k48` (ANSI outer-pinky home).
fn id_for(row: usize, col: usize) -> Option<String> {
    match (row, col) {
        (0, c) if c < 10 => Some(format!("main_k{}", c + 1)),
        (1, c) if c < 10 => Some(format!("main_k{}", c + 11)),
        (1, 10) => Some("main_k48".to_string()),
        (2, c) if c < 10 => Some(format!("main_k{}", c + 21)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    /// A minimal keyboard covering `main_k1..main_k30` so resolve
    /// tests can stay scoped to the mapping logic. Real keyboards
    /// live in `keyboards/*.json`.
    fn stub_keyboard(extra_k48: bool) -> Keyboard {
        use drift_core::{Finger, FingerColumn, Key, Row};
        let mut keys = HashMap::new();
        for n in 1..=30 {
            let id = KeyId::new(format!("main_k{n}"));
            keys.insert(
                id.clone(),
                Key {
                    id,
                    col: 0,
                    row: Row::Home,
                    x: 0.0,
                    y: 0.0,
                    finger: Finger::LIndex,
                    finger_column: FingerColumn::Outer,
                },
            );
        }
        if extra_k48 {
            let id = KeyId::new("main_k48");
            keys.insert(
                id.clone(),
                Key {
                    id,
                    col: 5,
                    row: Row::Home,
                    x: 0.0,
                    y: 0.0,
                    finger: Finger::RPinky,
                    finger_column: FingerColumn::Outer,
                },
            );
        }
        Keyboard { name: "stub".into(), keys }
    }

    #[test]
    fn colemak_maps_all_30_alpha_keys() {
        let doc = parse::from_str(
            r#"{"name":"colemak","board":"ortho","layers":{"main":[
                "q w f p g j l u y ;",
                "a r s t d h n e i o",
                "z x c v b k m , . /"
            ]}}"#,
        )
        .unwrap();
        let layout = resolve(&doc, &stub_keyboard(false)).unwrap();
        assert_eq!(layout.name, "colemak");
        // Every character in the .dof should be mapped.
        assert_eq!(layout.positions.len(), 30);
        assert_eq!(layout.position('q').unwrap().id.as_str(), "main_k1");
        assert_eq!(layout.position('a').unwrap().id.as_str(), "main_k11");
        assert_eq!(layout.position('z').unwrap().id.as_str(), "main_k21");
        assert_eq!(layout.position('o').unwrap().id.as_str(), "main_k20");
    }

    #[test]
    fn ansi_home_row_eleventh_char_lands_on_k48() {
        let doc = parse::from_str(
            r#"{"name":"halmak","board":"ansi","layers":{"main":[
                "w l r b z  ; q u d j",
                "s h n t ,  . a e o i '",
                "f m v c /  g p x k y"
            ]}}"#,
        )
        .unwrap();
        let layout = resolve(&doc, &stub_keyboard(true)).unwrap();
        assert_eq!(layout.position('\'').unwrap().id.as_str(), "main_k48");
    }

    #[test]
    fn keyboard_without_k48_silently_drops_eleventh_char() {
        let doc = parse::from_str(
            r#"{"name":"halmak","board":"ansi","layers":{"main":[
                "w l r b z  ; q u d j",
                "s h n t ,  . a e o i '",
                "f m v c /  g p x k y"
            ]}}"#,
        )
        .unwrap();
        let layout = resolve(&doc, &stub_keyboard(false)).unwrap();
        // Still resolves — just without the `'` slot.
        assert!(layout.position('\'').is_none());
        // Every other alpha still resolved.
        assert_eq!(layout.position('s').unwrap().id.as_str(), "main_k11");
    }

    #[test]
    fn uppercase_chars_lower_case_on_lookup() {
        let doc = parse::from_str(
            r#"{"name":"upper","board":"ortho","layers":{"main":[
                "Q W E R T Y U I O P",
                "A S D F G H J K L ;",
                "Z X C V B N M , . /"
            ]}}"#,
        )
        .unwrap();
        let layout = resolve(&doc, &stub_keyboard(false)).unwrap();
        assert_eq!(layout.position('q').unwrap().id.as_str(), "main_k1");
        assert_eq!(layout.position('Q').unwrap().id.as_str(), "main_k1");
    }
}
