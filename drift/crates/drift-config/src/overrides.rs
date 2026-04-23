//! CLI-level overrides on a loaded [`DriftConfig`].
//!
//! Lets callers tweak analyzer weights, or enable/disable analyzers,
//! without editing the underlying preset file. Overrides are applied
//! *after* a config is loaded and *before* the pipeline is built.
//!
//! Three operations:
//! * [`Override::Set`] — write a scalar at a dotted path
//!   (e.g. `sfb.penalty = -5.0`).
//! * [`Override::Enable`] — add an analyzer to the enabled list with
//!   default config.
//! * [`Override::Disable`] — remove an analyzer from the enabled list.
//!
//! Scalar values on `Set` are type-inferred in the order bool → i64
//! → f64 → string. That matches the natural use cases (`--set
//! foo.flag=true`, `--set sfb.penalty=-5.0`) without requiring the
//! user to think about TOML types.

use anyhow::{Context, Result, anyhow, bail};

use crate::DriftConfig;

/// One override to apply to a parsed config.
#[derive(Debug, Clone)]
pub enum Override {
    /// Set a scalar value at a dotted path in the config tree.
    /// The path is interpreted as nested TOML tables — e.g.
    /// `sfb.penalty` targets `analyzers.sfb.penalty` relative to the
    /// `analyzers` root. Override paths are always relative to
    /// `analyzers` (the only overridable subtree today).
    Set { path: String, value: String },
    /// Add an analyzer name to `[analyzers].enabled`. No-op if the
    /// name is already enabled.
    Enable(String),
    /// Remove an analyzer name from `[analyzers].enabled`. No-op if
    /// the name isn't currently enabled.
    Disable(String),
}

/// Apply every override in `overrides` to `config`, in order.
///
/// Mutates `config.raw` (for `Set` — writes scalar into the TOML
/// tree under `analyzers.<path>`) and `config.analyzer_names`
/// alongside the `enabled` array (for `Enable`/`Disable`).
pub fn apply(config: &mut DriftConfig, overrides: &[Override]) -> Result<()> {
    for ov in overrides {
        match ov {
            Override::Set { path, value } => apply_set(config, path, value)
                .with_context(|| format!("applying --set {path}={value}"))?,
            Override::Enable(name) => apply_enable(config, name)
                .with_context(|| format!("applying --enable {name}"))?,
            Override::Disable(name) => apply_disable(config, name)
                .with_context(|| format!("applying --disable {name}"))?,
        }
    }
    Ok(())
}

fn apply_set(config: &mut DriftConfig, path: &str, value: &str) -> Result<()> {
    let segments: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        bail!("override path is empty");
    }

    // All paths root under `analyzers`. We navigate there and create
    // missing tables on the way down.
    let analyzers = ensure_table(&mut config.raw, "analyzers")?;
    let mut cursor = analyzers;
    for seg in &segments[..segments.len() - 1] {
        cursor = ensure_table(cursor, seg)?;
    }

    let leaf = segments[segments.len() - 1];
    let tbl = cursor
        .as_table_mut()
        .ok_or_else(|| anyhow!("override parent is not a table"))?;
    tbl.insert(leaf.to_string(), parse_scalar(value));
    Ok(())
}

fn apply_enable(config: &mut DriftConfig, name: &str) -> Result<()> {
    if !config.analyzer_names.iter().any(|n| n == name) {
        config.analyzer_names.push(name.to_string());
    }
    let enabled = ensure_enabled_array(config)?;
    if !enabled.iter().any(|v| v.as_str() == Some(name)) {
        enabled.push(toml::Value::String(name.to_string()));
    }
    Ok(())
}

fn apply_disable(config: &mut DriftConfig, name: &str) -> Result<()> {
    config.analyzer_names.retain(|n| n != name);
    let enabled = ensure_enabled_array(config)?;
    enabled.retain(|v| v.as_str() != Some(name));
    Ok(())
}

/// Ensure `parent[key]` exists and is a table; return a mutable
/// reference to it, creating an empty table if the key was missing.
fn ensure_table<'a>(parent: &'a mut toml::Value, key: &str) -> Result<&'a mut toml::Value> {
    let tbl = parent
        .as_table_mut()
        .ok_or_else(|| anyhow!("config root is not a table"))?;
    if !tbl.contains_key(key) {
        tbl.insert(key.to_string(), toml::Value::Table(Default::default()));
    }
    let child = tbl.get_mut(key).expect("just inserted");
    if !child.is_table() {
        bail!("config path {key:?} is not a table");
    }
    Ok(child)
}

fn ensure_enabled_array(config: &mut DriftConfig) -> Result<&mut Vec<toml::Value>> {
    let analyzers = ensure_table(&mut config.raw, "analyzers")?;
    let tbl = analyzers
        .as_table_mut()
        .expect("ensure_table guarantees table");
    let entry = tbl
        .entry("enabled".to_string())
        .or_insert_with(|| toml::Value::Array(Vec::new()));
    entry
        .as_array_mut()
        .ok_or_else(|| anyhow!("[analyzers].enabled is not an array"))
}

/// Parse a CLI-provided scalar into a `toml::Value`. Tries bool,
/// then i64, then f64, else string. No quoting support — users who
/// want the literal string `"true"` or `"42"` will have to live
/// with the inference for now.
fn parse_scalar(s: &str) -> toml::Value {
    if let Ok(b) = s.parse::<bool>() {
        return toml::Value::Boolean(b);
    }
    if let Ok(i) = s.parse::<i64>() {
        return toml::Value::Integer(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        return toml::Value::Float(f);
    }
    toml::Value::String(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn config_with(raw: &str) -> DriftConfig {
        let raw: toml::Value = toml::from_str(raw).expect("valid toml");
        let analyzer_names = raw
            .get("analyzers")
            .and_then(|v| v.get("enabled"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        DriftConfig {
            corpus_path: PathBuf::new(),
            analyzer_names,
            raw,
        }
    }

    #[test]
    fn set_writes_scalar_into_nested_path() {
        let mut c = config_with(
            r#"
            [analyzers]
            enabled = ["sfb"]
            [analyzers.sfb]
            penalty = -7.0
        "#,
        );
        apply(
            &mut c,
            &[Override::Set {
                path: "sfb.penalty".into(),
                value: "-5.0".into(),
            }],
        )
        .unwrap();
        let v = c
            .raw
            .get("analyzers")
            .and_then(|v| v.get("sfb"))
            .and_then(|v| v.get("penalty"))
            .and_then(|v| v.as_float())
            .unwrap();
        assert_eq!(v, -5.0);
    }

    #[test]
    fn set_creates_missing_subtree() {
        let mut c = config_with(
            r#"
            [analyzers]
            enabled = []
        "#,
        );
        apply(
            &mut c,
            &[Override::Set {
                path: "new_one.weight".into(),
                value: "1.5".into(),
            }],
        )
        .unwrap();
        let v = c
            .raw
            .get("analyzers")
            .and_then(|v| v.get("new_one"))
            .and_then(|v| v.get("weight"))
            .and_then(|v| v.as_float())
            .unwrap();
        assert_eq!(v, 1.5);
    }

    #[test]
    fn scalar_parsing_infers_types() {
        assert_eq!(parse_scalar("true").as_bool(), Some(true));
        assert_eq!(parse_scalar("42").as_integer(), Some(42));
        assert_eq!(parse_scalar("-5.0").as_float(), Some(-5.0));
        assert_eq!(parse_scalar("hello").as_str(), Some("hello"));
    }

    #[test]
    fn enable_appends_and_deduplicates() {
        let mut c = config_with(
            r#"
            [analyzers]
            enabled = ["sfb"]
        "#,
        );
        apply(&mut c, &[Override::Enable("roll".into())]).unwrap();
        apply(&mut c, &[Override::Enable("roll".into())]).unwrap(); // second is no-op
        assert_eq!(c.analyzer_names, vec!["sfb", "roll"]);
        let enabled: Vec<&str> = c
            .raw
            .get("analyzers")
            .and_then(|v| v.get("enabled"))
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(enabled, vec!["sfb", "roll"]);
    }

    #[test]
    fn disable_removes_and_is_noop_when_absent() {
        let mut c = config_with(
            r#"
            [analyzers]
            enabled = ["sfb", "roll"]
        "#,
        );
        apply(&mut c, &[Override::Disable("sfb".into())]).unwrap();
        apply(&mut c, &[Override::Disable("sfb".into())]).unwrap(); // no-op
        assert_eq!(c.analyzer_names, vec!["roll"]);
        let enabled: Vec<&str> = c
            .raw
            .get("analyzers")
            .and_then(|v| v.get("enabled"))
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(enabled, vec!["roll"]);
    }

    #[test]
    fn empty_path_is_an_error() {
        let mut c = config_with(r#"[analyzers]
enabled = []"#);
        let err = apply(
            &mut c,
            &[Override::Set {
                path: "".into(),
                value: "1".into(),
            }],
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("empty"));
    }
}
