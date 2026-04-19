//! A single physical key — a switch at a position with a size and a
//! finger assignment.
//!
//! Identity is the `id` string. The id is opaque to the physical layer
//! itself (any unique-per-keyboard string works), but the wider project
//! follows naming conventions so layouts can target keys by known id.

use crate::physical::engine::{Cluster, Finger, Point, DEFAULT_CLUSTER};

/// One physical key.
///
/// Coordinates are in key-width units relative to home-row center:
/// - `x` grows right
/// - `y` grows down (home row at `y = 0`, top at `y = -1`, bottom at `y = 1`)
/// - `width`, `height` default to `1.0` (one key unit)
/// - `rotation` is in degrees clockwise, default `0.0`, pivoted at `(x, y)`
#[derive(Debug, Clone)]
pub struct PhysicalKey {
    /// Unique identifier within the owning keyboard. Layouts address
    /// keys by this id. See the project-wide naming convention for
    /// recommended forms (`main_k1`, `mods_shift_left`, etc.).
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
    pub cluster: Cluster,
    pub finger: Finger,
}

impl PhysicalKey {
    pub fn position(&self) -> Point {
        Point::new(self.x, self.y)
    }

    pub fn distance_to(&self, other: &PhysicalKey) -> f32 {
        self.position().distance_to(other.position())
    }
}

/// Default key size in key-width units.
pub const DEFAULT_SIZE: f32 = 1.0;

/// Default rotation in degrees.
pub const DEFAULT_ROTATION: f32 = 0.0;

/// Default cluster name. Re-exported so consumers outside the engine
/// module can use it without pulling in engine internals.
pub use crate::physical::engine::DEFAULT_CLUSTER as DEFAULT_CLUSTER_NAME;

/// Produce the default cluster as an owned [`Cluster`].
pub fn default_cluster() -> Cluster {
    DEFAULT_CLUSTER.to_string()
}
