//! Keyboard data model — hardware as first-class data.
//!
//! The `Keyboard` trait defines the abstract interface that every
//! keyboard implementation exposes. `common/` holds shared types
//! (PhysicalKey, Finger, Cluster, coordinates). `blocks/` is the
//! first concrete implementation — a keyboard is a sequence of
//! blocks, each with a stagger type.
//!
//! Alternative implementations (e.g. a flat-keys model, a radial
//! model) live as siblings under `keyboard/` and implement the same
//! trait. Engine and renderers go through the trait; they don't
//! reach into implementation details.
//!
//! See `docs/architecture-plan.md` and `docs/physical-model.md`.

pub mod blocks;
pub mod common;

pub use common::PhysicalKey;

use std::path::Path;

use common::Bounds;

/// The three structural stagger types a block can declare.
///
/// Each stagger type tells renderers how to lay out a block:
/// - `RowStag` — rows are rigid, can slide horizontally. ANSI keyboards.
///   Terminal honors fractional x offsets within a row.
/// - `ColStag` — columns are rigid, can slide vertically. Split ergo
///   boards. Terminal flattens y offsets; desktop renders them.
/// - `FreeForm` — no row/column rigidity. Fan-shaped thumb clusters.
///   Each key places itself via xy + rotation; terminal uses r/c.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaggerType {
    RowStag,
    ColStag,
    FreeForm,
}

/// A block — a logical region of a keyboard with its own stagger
/// type. Blocks have a cluster name (e.g. `"main"`, `"left_thumb"`)
/// that layouts use to address groups of keys, and they own a set
/// of `PhysicalKey`s.
pub trait Block {
    fn stagger_type(&self) -> StaggerType;
    /// Staged for future per-cluster theming / addressing.
    #[allow(dead_code)]
    fn cluster(&self) -> &str;
    fn keys(&self) -> Box<dyn Iterator<Item = &PhysicalKey> + '_>;
}

/// The abstract keyboard interface. Every concrete keyboard
/// implementation (blocks-based, flat, noblocks, …) implements this.
///
/// Consumers (engine, renderers, integrations) depend on this trait,
/// not on any concrete type.
///
/// `name` / `description` / `key` / `bounds` are staged for gui
/// renderers and integration tooling; today only `short` / `keys`
/// / `blocks` have callers.
#[allow(dead_code)]
pub trait Keyboard {
    fn name(&self) -> &str;
    fn short(&self) -> &str;
    fn description(&self) -> &str;

    /// Iterate every physical key on the board, regardless of which
    /// block or region it belongs to.
    fn keys(&self) -> Box<dyn Iterator<Item = &PhysicalKey> + '_>;

    /// Look up a key by its id. Used by the engine to resolve input
    /// events to physical keys.
    fn key(&self, id: &str) -> Option<&PhysicalKey>;

    /// Iterate the keyboard's blocks. Implementations without a
    /// block concept return an empty iterator; renderers fall back to
    /// a flat render for those.
    fn blocks(&self) -> Box<dyn Iterator<Item = &dyn Block> + '_>;

    /// Bounding box in geometric (x, y) space.
    fn bounds(&self) -> Bounds;
}

/// Load a keyboard from a JSON5 file. Dispatches to the right
/// implementation based on the file's declared schema (currently
/// only `blocks/` exists, so all files route there).
///
/// When future alternatives land, add a discriminator field to the
/// JSON schema (e.g. `"format": "blocks"` / `"flat"`) and branch here.
pub fn load(path: &Path) -> Result<Box<dyn Keyboard>, String> {
    let keyboard = blocks::loader::load(path)?;
    Ok(Box::new(keyboard))
}
