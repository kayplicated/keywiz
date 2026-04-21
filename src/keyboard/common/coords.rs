//! Coordinate primitives — the geometric vocabulary.
//!
//! All coordinates are in key-width units. Home row center = origin.
//! x grows right, y grows down.
//!
//! Staged for gui renderers (desktop, webui) and geometric analysis
//! (bigram distance, reach effort). The terminal renderer uses
//! schematic `r`/`c` instead.

#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance_to(self, other: Point) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}

impl Bounds {
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
