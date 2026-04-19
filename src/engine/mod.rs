//! Runtime keyboard coordinator.
//!
//! Owns the active keyboard (as `Box<dyn Keyboard>`) and layout,
//! plus the catalog of alternatives for cycling. Input processing
//! (char → id lookup via the layout, hit/miss dispatch) lives here
//! so renderers and modes depend on one coordinator, not a mix of
//! translate/grid/manager like the old architecture.
//!
//! Broken JSONs (unparseable keyboards/layouts) stay in the catalog
//! marked broken; the UI colors them red and the active state stays
//! on the previous working selection.

pub mod catalog;
pub mod state;
pub mod translate;

pub use state::{BrokenSelection, Engine, EngineError, LayoutChange};
pub use translate::Translator;
