//! Small helpers for reading per-rule TOML subtables.

use toml::Value;

/// Read a numeric field from an optional rule subtable. Missing
/// tables and missing keys both fall back to `default`. Accepts
/// integer and float literals in TOML.
pub fn read_f64(sub: Option<&Value>, key: &str, default: f64) -> f64 {
    sub.and_then(|t| t.get(key))
        .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
        .unwrap_or(default)
}
