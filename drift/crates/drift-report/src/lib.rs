//! Renderers for [`ScoreResult`](drift_score::ScoreResult).
//!
//! One renderer per backend. The CLI picks via flag; a future web
//! UI would depend on the same types and render differently.

use drift_score::ScoreResult;

pub mod aggregate;
pub mod diff;
pub mod json;
pub mod text;

/// Renders a score result to an owned string in some format.
pub trait Renderer {
    fn render(&self, result: &ScoreResult) -> String;
}
