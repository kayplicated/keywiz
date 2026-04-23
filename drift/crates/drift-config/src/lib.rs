//! TOML-backed config loader for drift.
//!
//! Reads a drift config file, resolves the `[analyzers].enabled`
//! list against a [`Registry`](drift_analyzer::Registry), and
//! builds a [`Pipeline`](drift_analyzer::Pipeline). Also manages
//! preset resolution.
//!
//! This crate provides the `toml`-backed implementation of
//! drift-analyzer's [`ConfigValue`](drift_analyzer::ConfigValue)
//! trait, so analyzer authors read values through a neutral
//! abstraction regardless of the concrete config format.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use drift_analyzer::{ConfigValue, Pipeline, PipelineBuilder, Registry};

pub mod overrides;
pub mod presets;
pub mod toml_value;

pub use overrides::{Override, apply as apply_overrides};
pub use toml_value::TomlValue;

/// Parsed drift config — retains enough structure that the pipeline
/// builder can look up analyzer-specific subtrees by name.
pub struct DriftConfig {
    /// Corpus file path (interpreted relative to the config file's
    /// directory if relative).
    pub corpus_path: PathBuf,
    /// Ordered list of analyzer names to enable.
    pub analyzer_names: Vec<String>,
    /// Raw root of the parsed TOML. Analyzer-specific subtrees are
    /// looked up under `[analyzers.<name>]`.
    pub raw: toml::Value,
}

/// Load a config file from disk.
pub fn load(path: &Path) -> Result<DriftConfig> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading config: {}", path.display()))?;
    let raw: toml::Value = toml::from_str(&text)
        .with_context(|| format!("parsing config: {}", path.display()))?;

    let analyzer_names = extract_analyzer_names(&raw)
        .context("reading [analyzers].enabled")?;
    let corpus_path = extract_corpus_path(&raw, path)
        .context("reading [corpus].path")?;

    Ok(DriftConfig {
        corpus_path,
        analyzer_names,
        raw,
    })
}

/// Load one of the bundled presets.
pub fn load_preset(name: &str) -> Result<DriftConfig> {
    let path = presets::path_for(name)?;
    load(&path)
}

/// Materialize a pipeline from a parsed config + registry.
///
/// For each name in `config.analyzer_names`, looks up the analyzer
/// in `registry`, extracts its `[analyzers.<name>]` subtree (if
/// present), and invokes the constructor.
pub fn build_pipeline(config: &DriftConfig, registry: &Registry) -> Result<Pipeline> {
    let mut builder = PipelineBuilder::new();
    for name in &config.analyzer_names {
        let sub = analyzer_subtree(&config.raw, name);
        let sub_ref: Option<&dyn ConfigValue> = sub.as_ref().map(|v| v as &dyn ConfigValue);
        let analyzer = registry
            .build(name, sub_ref)
            .with_context(|| format!("constructing analyzer {name:?}"))?;
        builder.push(analyzer);
    }
    Ok(builder.build())
}

fn extract_analyzer_names(raw: &toml::Value) -> Result<Vec<String>> {
    let analyzers = raw
        .get("analyzers")
        .and_then(|v| v.as_table())
        .ok_or_else(|| anyhow!("[analyzers] section is missing"))?;
    let enabled = analyzers
        .get("enabled")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("[analyzers].enabled must be a list of strings"))?;
    enabled
        .iter()
        .map(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow!("[analyzers].enabled entries must be strings"))
        })
        .collect()
}

fn extract_corpus_path(raw: &toml::Value, config_path: &Path) -> Result<PathBuf> {
    let corpus = raw
        .get("corpus")
        .and_then(|v| v.as_table())
        .ok_or_else(|| anyhow!("[corpus] section is missing"))?;
    let p = corpus
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("[corpus].path must be a string"))?;

    let path = PathBuf::from(p);
    if path.is_absolute() {
        Ok(path)
    } else if let Some(parent) = config_path.parent() {
        Ok(parent.join(path))
    } else {
        Ok(path)
    }
}

/// Extract the `[analyzers.<name>]` subtree, if present, as an
/// owned TomlValue ready to hand to an analyzer constructor.
fn analyzer_subtree(raw: &toml::Value, name: &str) -> Option<TomlValue> {
    raw.get("analyzers")
        .and_then(|v| v.as_table())
        .and_then(|t| t.get(name))
        .cloned()
        .map(TomlValue)
}
