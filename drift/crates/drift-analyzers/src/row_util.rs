//! Row-index arithmetic helpers.
//!
//! Analyzers that need to compute row deltas (how many rows apart
//! are these two keys?) use [`row_index`] to map named rows into
//! integers. Top = -1, Home = 0, Bottom = 1, matching the original
//! keywiz convention.

use drift_core::Row;

/// Integer index for arithmetic. Unknown variants map to 0
/// (home-equivalent) — analyzers that care about a specific new
/// row should pattern-match on `Row` directly.
pub fn row_index(row: Row) -> i32 {
    match row {
        Row::Number => -2,
        Row::Top => -1,
        Row::Home => 0,
        Row::Bottom => 1,
        Row::Extra(n) => 2 + i32::from(n),
        _ => 0,
    }
}

/// Parse a row name from config — case-insensitive. Returns `None`
/// for unknown names so analyzers can surface a helpful error, or
/// silently drop the entry if they prefer permissive parsing.
pub fn parse_row_name(name: &str) -> Option<Row> {
    match name.to_ascii_lowercase().as_str() {
        "top" => Some(Row::Top),
        "home" => Some(Row::Home),
        "bottom" | "bot" => Some(Row::Bottom),
        "number" | "num" => Some(Row::Number),
        _ => None,
    }
}
