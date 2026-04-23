//! Shared vocabulary for the drift layout-analysis framework.
//!
//! This crate holds the types every other drift crate speaks in:
//! fingers, keys, keyboards, layouts, the corpus trait, and the
//! hit/scope types that analyzers produce and consume. It has no
//! I/O and no logic beyond trivial accessors. Loaders live in
//! drift-keyboard / drift-corpus; the analyzer trait lives in
//! drift-analyzer; scoring lives in drift-score.
//!
//! Third-party analyzer crates depend on this and drift-analyzer.

pub mod corpus;
pub mod finger;
pub mod hit;
pub mod key;
pub mod keyboard;
pub mod layout;
pub mod row;
pub mod scope;
pub mod window;

pub use corpus::CorpusSource;
pub use finger::{Finger, Hand};
pub use hit::Hit;
pub use key::{FingerColumn, Key, KeyId};
pub use keyboard::Keyboard;
pub use layout::Layout;
pub use row::Row;
pub use scope::Scope;
pub use window::{Window, WindowProps};
