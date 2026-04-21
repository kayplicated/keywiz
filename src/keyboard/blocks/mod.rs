//! Blocks-based keyboard implementation.
//!
//! A keyboard is a sequence of `BlockKind` values, each owning a
//! cluster name and a set of keys. `BlocksKeyboard` implements
//! `trait Keyboard`.
//!
//! The three concrete block structs (`RowStagBlock`, `ColStagBlock`,
//! `FreeFormBlock`) carry identical fields today — they differ only
//! in the `StaggerType` they return. Kept separate because the gui
//! renderer is expected to grow per-kind fields (row-stag per-row
//! shift overrides, col-stag per-column splay profiles, free-form
//! rotation anchors / curve radii). When those fields land they
//! belong in one struct each, not on a shared struct with Option
//! fields.

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
