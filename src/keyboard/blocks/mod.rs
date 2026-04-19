//! Blocks-based keyboard implementation.
//!
//! A keyboard is a sequence of `BlockKind` values, each owning a
//! cluster name and a set of keys. `BlocksKeyboard` implements
//! `trait Keyboard`.

pub mod block;
pub mod colstag;
pub mod freeform;
pub mod keyboard;
pub mod loader;
pub mod rowstag;

pub use block::BlockKind;
pub use colstag::ColStagBlock;
pub use freeform::FreeFormBlock;
pub use keyboard::BlocksKeyboard;
pub use rowstag::RowStagBlock;
