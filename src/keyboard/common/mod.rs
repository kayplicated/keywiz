//! Shared vocabulary used by every `trait Keyboard` implementation.
//!
//! Each implementation (blocks, future flat, future noblocks) uses
//! the same `PhysicalKey`, `Finger`, `Cluster`, and coordinate types.
//! This module is the lingua franca.

pub mod cluster;
pub mod coords;
pub mod finger;
pub mod key;

pub use cluster::Cluster;
pub use coords::{Bounds, Point};
pub use finger::Finger;
pub use key::PhysicalKey;
