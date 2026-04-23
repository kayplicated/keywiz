//! `ConfigValue` implementation over `toml::Value`.
//!
//! `TomlValue` wraps an owned `toml::Value`. Child lookups clone
//! the relevant subtree — cheap at config-build time, which is the
//! only time analyzer configuration is read.

use drift_analyzer::ConfigValue;

/// Owned adapter over a `toml::Value`.
pub struct TomlValue(pub toml::Value);

impl ConfigValue for TomlValue {
    fn get(&self, key: &str) -> Option<Box<dyn ConfigValue>> {
        let child = self.0.as_table()?.get(key)?.clone();
        Some(Box::new(TomlValue(child)))
    }

    fn as_f64(&self) -> Option<f64> {
        self.0
            .as_float()
            .or_else(|| self.0.as_integer().map(|i| i as f64))
    }

    fn as_i64(&self) -> Option<i64> {
        self.0.as_integer()
    }

    fn as_bool(&self) -> Option<bool> {
        self.0.as_bool()
    }

    fn as_str(&self) -> Option<&str> {
        self.0.as_str()
    }

    fn as_array(&self) -> Option<Vec<Box<dyn ConfigValue>>> {
        let arr = self.0.as_array()?;
        Some(
            arr.iter()
                .cloned()
                .map(|v| Box::new(TomlValue(v)) as Box<dyn ConfigValue>)
                .collect(),
        )
    }
}
