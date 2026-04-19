//! Physical keyboard model — hardware as first-class data.
//!
//! Two layers:
//! - [`engine`] — coordinate primitives, clusters, fingers. Pure math
//!   and vocabulary, independent of any specific key.
//! - [`keys`] — [`PhysicalKey`] and [`PhysicalKeyboard`] built on top
//!   of the engine. Describes real switches at real positions.
//!
//! No JSON parsing here — that lives in [`crate::configreader`]. No
//! character mapping — that lives in [`crate::grid::layout`]. The
//! physical layer knows only: "these switches exist, they sit at these
//! positions, and these fingers press them."

pub mod engine;
pub mod keys;

pub use engine::{Cluster, Finger};
pub use keys::{human_name, PhysicalKey, PhysicalKeyboard};
