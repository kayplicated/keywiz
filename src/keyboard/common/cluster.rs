//! Cluster names — logical groupings of keys within a keyboard.
//!
//! Suggested conventions are documented in `docs/physical-model.md`.
//! The physical layer treats the cluster as opaque string metadata.

pub type Cluster = String;

/// Default cluster for blocks that don't declare one. Staged for
/// future use; current loaders require an explicit cluster per block.
#[allow(dead_code)]
pub const DEFAULT_CLUSTER: &str = "main";
