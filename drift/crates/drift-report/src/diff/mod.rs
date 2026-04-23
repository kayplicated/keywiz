//! Layout-diff rendering.
//!
//! Given two [`Layout`](drift_core::Layout)s over the same
//! [`Keyboard`](drift_core::Keyboard), produce a per-key diff
//! showing which characters moved. Format-agnostic computation
//! lives in [`compute`]; renderers live in [`text`] and [`json`].

pub mod compute;
pub mod json;
pub mod text;
