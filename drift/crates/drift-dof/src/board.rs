//! Default keywiz keyboard files per `.dof` board descriptor.
//!
//! A `.dof` file names a board (`ortho`, `elora`, `ansi`) but
//! carries no geometry. This module maps that name to a keywiz
//! keyboard JSON under `keyboards/` whose geometry matches the
//! intent of the descriptor. Callers can always override by
//! passing `--keyboard` explicitly; this is only the default.
//!
//! The returned path is a suggestion relative to the repo root,
//! not a verified existence claim — the loader still has to
//! actually read the file.

/// Default keyboard path for a `.dof` board descriptor.
///
/// Returns `None` for unknown descriptors so the caller can
/// decide whether to error, fall back to a repo-wide default, or
/// ask for `--keyboard`.
pub fn default_keyboard_path(board: &str) -> Option<&'static str> {
    match board {
        "ortho" => Some("keyboards/ortho.json"),
        "elora" => Some("keyboards/halcyon_elora.json"),
        "ansi" => Some("keyboards/us_intl.json"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_boards_resolve() {
        assert_eq!(default_keyboard_path("ortho"), Some("keyboards/ortho.json"));
        assert_eq!(
            default_keyboard_path("elora"),
            Some("keyboards/halcyon_elora.json")
        );
        assert_eq!(
            default_keyboard_path("ansi"),
            Some("keyboards/us_intl.json")
        );
    }

    #[test]
    fn unknown_board_is_none() {
        assert_eq!(default_keyboard_path("foo"), None);
        assert_eq!(default_keyboard_path(""), None);
    }
}
