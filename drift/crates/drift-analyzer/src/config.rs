//! The `ConfigValue` trait — drift-analyzer's format-agnostic view
//! of analyzer configuration.
//!
//! Analyzer constructors read their parameters through this trait
//! rather than depending on `toml::Value` or `serde_json::Value`
//! directly. drift-config supplies the TOML implementation;
//! alternative config formats can provide their own without any
//! analyzer-side change.

/// A read-only view of one analyzer's configuration subtree.
///
/// Methods return `Option` so missing values can fall back to the
/// analyzer's defaults. Wrong-type accesses also return `None`;
/// analyzers that want strict validation should check the type
/// explicitly and error themselves.
///
/// Child-returning methods (`get`, `as_array`) return boxed owned
/// adapters rather than borrowed references. This is deliberate:
/// concrete adapters (TOML, JSON) often want to clone sub-trees
/// rather than maintain parent-child borrow chains, and the
/// performance cost is irrelevant because config is parsed once
/// at startup.
pub trait ConfigValue {
    /// Get a child value by key. Returns `None` if the key is
    /// missing, or if `self` isn't a table.
    fn get(&self, key: &str) -> Option<Box<dyn ConfigValue>>;

    /// Read as a 64-bit float. Missing / wrong-type returns `None`.
    fn as_f64(&self) -> Option<f64>;

    /// Read as a 64-bit signed integer.
    fn as_i64(&self) -> Option<i64>;

    /// Read as a boolean.
    fn as_bool(&self) -> Option<bool>;

    /// Read as a string slice. Shares self's lifetime.
    fn as_str(&self) -> Option<&str>;

    /// Collect array entries as owned adapters. Returns `None` if
    /// `self` isn't an array.
    fn as_array(&self) -> Option<Vec<Box<dyn ConfigValue>>>;
}

/// Read a named f64 from an optional config subtree, or fall back
/// to `default` if the key is missing or of the wrong type.
/// Convenience for the common analyzer-config pattern.
pub fn f64_or(cfg: Option<&dyn ConfigValue>, key: &str, default: f64) -> f64 {
    cfg.and_then(|c| c.get(key))
        .and_then(|v| v.as_f64())
        .unwrap_or(default)
}

/// Read a named bool from an optional config subtree, or fall back
/// to `default`.
pub fn bool_or(cfg: Option<&dyn ConfigValue>, key: &str, default: bool) -> bool {
    cfg.and_then(|c| c.get(key))
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

/// Read a named array-of-strings from config, or fall back to
/// `default`. Non-string entries in the array are skipped (not an
/// error) so analyzers can accept mixed-type arrays defensively.
pub fn strings_or(cfg: Option<&dyn ConfigValue>, key: &str, default: &[&str]) -> Vec<String> {
    cfg.and_then(|c| c.get(key))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_else(|| default.iter().map(|s| s.to_string()).collect())
}
