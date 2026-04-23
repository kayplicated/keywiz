//! `.dof` JSON parser.
//!
//! Maps the oxeylyzer document schema to a plain Rust struct. The
//! schema we accept:
//!
//! ```json
//! {
//!   "name": "...",
//!   "board": "ortho" | "elora" | "ansi",
//!   "layers": { "main": ["row1", "row2", "row3"] },
//!   "fingering": "traditional",
//!   "authors": [...],   // optional, ignored
//!   "year": 2024        // optional, ignored
//! }
//! ```

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

/// Parsed `.dof` document. Only fields drift uses are retained;
/// `authors`, `year`, `fingering` are accepted during parsing but
/// not surfaced — they're metadata the scorer doesn't consume.
#[derive(Debug, Clone)]
pub struct DofDocument {
    pub name: String,
    pub board: String,
    /// The three rows of alpha characters, whitespace-separated in
    /// the source and split into `Vec<String>` here. Each entry is
    /// one character (single grapheme) in the usual case; parsers
    /// downstream take the first `char` of each entry.
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct Raw {
    name: String,
    board: String,
    layers: RawLayers,
    #[serde(default)]
    _fingering: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawLayers {
    main: Vec<String>,
}

/// Parse a `.dof` file on disk.
pub fn load(path: &Path) -> Result<DofDocument> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading .dof: {}", path.display()))?;
    from_str(&text).with_context(|| format!("parsing .dof: {}", path.display()))
}

/// Parse a `.dof` document from a string.
pub fn from_str(text: &str) -> Result<DofDocument> {
    let raw: Raw = serde_json::from_str(text).context("parsing .dof JSON")?;
    if raw.layers.main.len() != 3 {
        return Err(anyhow!(
            "expected 3 rows in layers.main, got {}",
            raw.layers.main.len()
        ));
    }
    let rows: Vec<Vec<String>> = raw
        .layers
        .main
        .iter()
        .map(|row| row.split_whitespace().map(str::to_string).collect())
        .collect();
    Ok(DofDocument {
        name: raw.name,
        board: raw.board,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_colemak() {
        let doc = from_str(
            r#"{"name":"colemak","board":"ortho","layers":{"main":[
                "q w f p g j l u y ;",
                "a r s t d h n e i o",
                "z x c v b k m , . /"
            ]},"fingering":"traditional"}"#,
        )
        .unwrap();
        assert_eq!(doc.name, "colemak");
        assert_eq!(doc.board, "ortho");
        assert_eq!(doc.rows.len(), 3);
        assert_eq!(doc.rows[1], vec!["a", "r", "s", "t", "d", "h", "n", "e", "i", "o"]);
    }

    #[test]
    fn parses_halmak_ansi_with_11_char_home_row() {
        let doc = from_str(
            r#"{"name":"halmak","board":"ansi","layers":{"main":[
                "w l r b z  ; q u d j",
                "s h n t ,  . a e o i '",
                "f m v c /  g p x k y"
            ]},"fingering":"traditional"}"#,
        )
        .unwrap();
        assert_eq!(doc.rows[1].len(), 11);
        assert_eq!(doc.rows[1].last().unwrap(), "'");
    }

    #[test]
    fn rejects_wrong_row_count() {
        let err = from_str(
            r#"{"name":"x","board":"ortho","layers":{"main":["a b c"]}}"#,
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("3 rows"));
    }

    #[test]
    fn accepts_optional_metadata() {
        // authors and year are present in some real files; they
        // must not break parsing.
        let doc = from_str(
            r#"{"name":"drifter","board":"elora","authors":["kay"],"year":2026,
                "layers":{"main":[
                    "q v m j ;  , - = x z",
                    "n r t s g  p h e a i",
                    "b l d c w  k f u o y"
                ]}}"#,
        )
        .unwrap();
        assert_eq!(doc.name, "drifter");
        assert_eq!(doc.rows[0].len(), 10);
    }
}
