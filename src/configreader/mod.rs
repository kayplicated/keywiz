//! Pluggable readers for external keyboard-config formats.
//!
//! Each format (kanata, qmk, zmk, …) lives in its own submodule and
//! implements [`ConfigReader`]. The reader's job is to turn a config file
//! into a [`Grid`](crate::grid::Grid) that the rest of the app can
//! consume — same data shape as the JSON-driven path, so modes, the
//! widget, the heatmap, and the manager don't care where the grid came
//! from.
//!
//! Adding a new format = a new file in this directory + a one-line entry
//! in `dispatch_by_extension` (or whatever the caller chooses to use).

pub mod kanata;
pub mod keyboard;
pub mod layout;

use crate::grid::Grid;

/// Errors a reader can return. Kept simple; readers convert their own
/// internal errors into a string message for the user.
#[derive(Debug)]
pub enum ReaderError {
    /// File could not be opened or read. Reserved for readers that do
    /// their own I/O — the kanata reader takes pre-loaded text and never
    /// returns this variant.
    #[allow(dead_code)]
    Io(String),
    /// File parsed but the requested layer/keymap doesn't exist.
    UnknownLayer { name: String, available: Vec<String> },
    /// Source content was malformed for this format.
    Malformed(String),
}

impl std::fmt::Display for ReaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReaderError::Io(msg) => write!(f, "{msg}"),
            ReaderError::UnknownLayer { name, available } => {
                write!(
                    f,
                    "no such layer '{name}'; available: {}",
                    available.join(", ")
                )
            }
            ReaderError::Malformed(msg) => write!(f, "malformed config: {msg}"),
        }
    }
}

/// A reader that parses a specific keyboard-config format and produces a
/// [`Grid`]. Implementations live in submodules of [`crate::configreader`].
pub trait ConfigReader {
    /// Human-readable name of the format ("kanata", "qmk", …). Used in
    /// error messages and help text.
    fn format_name(&self) -> &'static str;

    /// Parse `source` and return a [`Grid`]. `selector` picks one layer or
    /// keymap when the format supports multiple; `None` should pick a
    /// sensible default (e.g. the first declared layer).
    fn read(&self, source: &str, selector: Option<&str>) -> Result<Grid, ReaderError>;

    /// List the named layers/keymaps in `source`, in declaration order.
    /// Empty if the format has no concept of named layers. Reserved for a
    /// future "list layers" CLI subcommand.
    #[allow(dead_code)]
    fn list_layers(&self, source: &str) -> Vec<String>;
}
