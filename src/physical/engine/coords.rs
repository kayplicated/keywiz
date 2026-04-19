//! Coordinate primitives for the physical keyboard model.
//!
//! All coordinates are in key-width units relative to home-row center:
//! - `x` grows right
//! - `y` grows down (home row at y=0, top at y=-1, bottom at y=1)
//! - one unit = one standard key width (= one standard key height)
//!
//! The engine is pure math — no ids, no fingers, no rendering. It gives
//! the rest of the physical model a vocabulary for "where is something
//! and how big is it."

/// A 2D point in key-width units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Euclidean distance to another point, in key-width units.
    pub fn distance_to(self, other: Point) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// An axis-aligned bounding box in key-width units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}

impl Bounds {
    /// Smallest bounds containing all points. Empty input yields an empty
    /// box at the origin.
    pub fn enclosing<I: IntoIterator<Item = Point>>(points: I) -> Self {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        let mut any = false;
        for p in points {
            any = true;
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }
        if !any {
            return Bounds {
                min: Point::new(0.0, 0.0),
                max: Point::new(0.0, 0.0),
            };
        }
        Bounds {
            min: Point::new(min_x, min_y),
            max: Point::new(max_x, max_y),
        }
    }

    pub fn width(self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(self) -> f32 {
        self.max.y - self.min.y
    }
}
