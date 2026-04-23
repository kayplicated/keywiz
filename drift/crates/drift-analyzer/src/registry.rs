//! The `Registry` — analyzer-name → constructor lookup.
//!
//! Each analyzer type calls `Registry::register` once at startup
//! (typically in a per-analyzer `register` free function). The
//! registry is consumed by drift-config to build a `Pipeline` from
//! the user's `[analyzers].enabled` list.

use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::{Analyzer, ConfigValue};

/// Constructor signature shared by all analyzer types.
pub type AnalyzerBuilder = fn(Option<&dyn ConfigValue>) -> Result<Box<dyn Analyzer>>;

/// A registered analyzer type. Held by the registry; never
/// invoked directly by user code.
pub struct AnalyzerEntry {
    /// Stable name. Must match the analyzer's `Analyzer::name()`.
    pub name: &'static str,

    /// Constructor. Receives the analyzer's config subtree (or
    /// `None` if the user didn't supply one) and returns the
    /// built analyzer or an error.
    pub build: AnalyzerBuilder,
}

/// Registry of known analyzer types.
///
/// Populated at startup by stock and third-party analyzer crates.
/// Queried by drift-config to materialize a pipeline from config
/// entries.
pub struct Registry {
    entries: HashMap<&'static str, AnalyzerEntry>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Register an analyzer type. Panics on duplicate names —
    /// duplicate registration is a programmer error, not a runtime
    /// condition.
    pub fn register(&mut self, entry: AnalyzerEntry) {
        let name = entry.name;
        if self.entries.insert(name, entry).is_some() {
            panic!("analyzer {name:?} registered twice");
        }
    }

    /// Build an analyzer instance by name, passing the given config
    /// subtree to its constructor.
    pub fn build(
        &self,
        name: &str,
        config: Option<&dyn ConfigValue>,
    ) -> Result<Box<dyn Analyzer>> {
        let entry = self
            .entries
            .get(name)
            .ok_or_else(|| anyhow!("unknown analyzer: {name:?}"))?;
        (entry.build)(config)
    }

    /// Iterate over the names of all registered analyzers. Useful
    /// for listing in CLI help or diagnostic output.
    pub fn known(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.keys().copied()
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}
