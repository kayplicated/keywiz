//! Cluster names — logical groupings of keys within a keyboard.
//!
//! Suggested conventions are documented in `docs/physical-model.md`.
//! The physical layer treats the cluster as opaque string metadata.

pub type Cluster = String;

pub const DEFAULT_CLUSTER: &str = "main";
