//! The physical keyboard's coordinate system and vocabulary.
//!
//! Pure model — coordinates, clusters, finger assignments. No keys here,
//! no JSON parsing, no rendering. The engine gives
//! [`crate::physical::keys`] the types it needs to describe individual
//! switches.

pub mod cluster;
pub mod coords;
pub mod finger;

pub use cluster::{Cluster, DEFAULT_CLUSTER};
pub use coords::{Bounds, Point};
pub use finger::Finger;
