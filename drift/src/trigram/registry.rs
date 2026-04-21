//! Builds the active trigram pipeline from config.
//!
//! Each rule type is matched by name; the matching constructor
//! reads its own sub-table under `[trigram.<rule-name>]` and
//! returns an initialized rule. Unknown names are an error so
//! typos are caught early.

use anyhow::{anyhow, Context, Result};
use toml::Value;

use super::rule::TrigramRule;
use super::rules;

/// Ordered list of rules that apply to every trigram.
pub struct TrigramPipeline {
    pub rules: Vec<Box<dyn TrigramRule>>,
}

impl TrigramPipeline {
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// Build a pipeline from the `[trigram]` section of `drift.toml`.
///
/// The section looks like:
/// ```toml
/// [trigram]
/// rules = ["inward_roll", "outward_roll", ...]
///
/// [trigram.inward_roll]
/// weight = 3.0
/// end_on_pinky_multiplier = 0.5
/// ```
pub fn build_pipeline(trigram_table: &Value) -> Result<TrigramPipeline> {
    let Some(rules_value) = trigram_table.get("rules") else {
        return Ok(TrigramPipeline::empty());
    };
    let rule_names = rules_value
        .as_array()
        .ok_or_else(|| anyhow!("[trigram].rules must be a list of rule names"))?;

    let mut rules: Vec<Box<dyn TrigramRule>> = Vec::with_capacity(rule_names.len());
    for name_val in rule_names {
        let name = name_val
            .as_str()
            .ok_or_else(|| anyhow!("[trigram].rules entries must be strings"))?;
        let sub = trigram_table.get(name);
        let rule = construct_rule(name, sub)
            .with_context(|| format!("constructing trigram rule {name:?}"))?;
        rules.push(rule);
    }

    Ok(TrigramPipeline { rules })
}

/// Dispatch on rule name to the appropriate constructor.
///
/// Add new rules here when adding files under `rules/`.
fn construct_rule(name: &str, sub: Option<&Value>) -> Result<Box<dyn TrigramRule>> {
    match name {
        "inward_roll" => Ok(Box::new(rules::roll::InwardRoll::from_config(sub)?)),
        "outward_roll" => Ok(Box::new(rules::roll::OutwardRoll::from_config(sub)?)),
        "onehand" => Ok(Box::new(rules::onehand::Onehand::from_config(sub)?)),
        "alternate" => Ok(Box::new(rules::alternate::Alternate::from_config(sub)?)),
        "redirect" => Ok(Box::new(rules::redirect::Redirect::from_config(sub)?)),
        "bad_redirect" => Ok(Box::new(rules::redirect::BadRedirect::from_config(sub)?)),
        "pinky_terminal" => Ok(Box::new(rules::pinky_terminal::PinkyTerminal::from_config(
            sub,
        )?)),
        "flexion_cascade" => Ok(Box::new(rules::flexion_cascade::FlexionCascade::from_config(
            sub,
        )?)),
        "row_cascade" => Ok(Box::new(rules::row_cascade::RowCascade::from_config(sub)?)),
        other => Err(anyhow!("unknown trigram rule: {other}")),
    }
}
