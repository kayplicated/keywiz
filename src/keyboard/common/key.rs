//! A physical key — a switch at a position with a finger assignment.
//!
//! Carries both coordinate systems per `docs/physical-model.md`:
//! - `r`, `c`: schematic integer row/column for terminal rendering
//! - `x`, `y`, `width`, `height`, `rotation`: geometric position for
//!   desktop/webui rendering and distance analysis
//!
//! All implementations of `trait Keyboard` use this same type. It's
//! shared vocabulary — moving it between implementations is a no-op.

use crate::keyboard::common::{Cluster, Finger, Point};

#[derive(Debug, Clone)]
pub struct PhysicalKey {
    pub id: String,
    /// Schematic row index. Integer. Used by terminal renderer.
    pub r: i32,
    /// Schematic column index. Integer. Used by terminal renderer.
    pub c: i32,
    /// Geometric x (key-width units from home-row center). Used by
    /// desktop/webui and distance analysis.
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
