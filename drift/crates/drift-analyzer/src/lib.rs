//! The Analyzer trait and pipeline abstractions.
//!
//! An analyzer is a pluggable scoring module. Each one declares a
//! [`Scope`](drift_core::Scope) — what window shape it consumes —
//! and emits zero or more [`Hit`](drift_core::Hit)s per window.
//!
//! This crate defines three public surfaces:
//!
//! - [`Analyzer`]: the trait analyzer authors implement.
//! - [`Registry`]: maps analyzer names to constructors. Stock
//!   analyzers (in drift-analyzers) and third-party analyzers both
//!   register into this.
//! - [`Pipeline`]: an ordered set of analyzer instances, built from
//!   config + registry. Consumed by drift-score.
//!
//! Analyzers read their configuration through the [`ConfigValue`]
//! trait, not any concrete config format. This keeps drift-analyzer
//! free of `toml`, `serde`, or other format-specific dependencies —
//! drift-config provides the TOML implementation.

pub mod aggregate;
pub mod analyzer;
pub mod config;
pub mod pipeline;
pub mod registry;

pub use aggregate::AggregateContext;
pub use analyzer::Analyzer;
pub use config::{bool_or, f64_or, strings_or, ConfigValue};
pub use pipeline::{Pipeline, PipelineBuilder};
pub use registry::{AnalyzerEntry, Registry};
