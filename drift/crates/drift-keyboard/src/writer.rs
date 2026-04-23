//! Layout writer — serializes a [`Layout`] back to the keywiz
//! JSON5 schema so generated layouts can be saved and re-used.
//!
//! The output is valid JSON5 but deliberately readable: alpha-core
//! bindings are sorted by key id so diffs between successive SA
//! runs are easy to read. Named keys (shift, tab, enter, etc.)
//! aren't emitted because drift doesn't see them — if the generated
//! layout is going to be used in keywiz itself, hand-merge the
//! named-key block from a template.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;

use anyhow::{Context, Result};
use drift_core::{Key, Layout};

/// Serialize a layout to the keywiz JSON5 schema as a string.
///
/// `short` is an optional short name (displayed in the keywiz UI);
/// if `None`, uses the layout's `name` verbatim.
pub fn to_json5(layout: &Layout, short: Option<&str>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{{");
    let _ = writeln!(out, "  name: \"{}\",", escape(&layout.name));
    let _ = writeln!(
        out,
        "  short: \"{}\",",
        escape(short.unwrap_or(&layout.name))
    );
    let _ = writeln!(out, "  mappings: {{");

    // Sort by key id so diffs between runs are stable.
    let mut sorted: BTreeMap<&str, (&char, &Key)> = BTreeMap::new();
    for (ch, key) in &layout.positions {
        sorted.insert(key.id.as_str(), (ch, key));
    }

    for (key_id, (ch, _)) in &sorted {
        let lower = ch.to_ascii_lowercase();
        let upper = ch.to_ascii_uppercase();
        let _ = writeln!(
            out,
            "    {}: {{ char: [\"{}\", \"{}\"] }},",
            key_id,
            escape_char(lower),
            escape_char(upper)
        );
    }

    let _ = writeln!(out, "  }},");
    let _ = writeln!(out, "}}");
    out
}

/// Write a layout to a file in keywiz JSON5 format.
pub fn write(path: &Path, layout: &Layout, short: Option<&str>) -> Result<()> {
    let text = to_json5(layout, short);
    std::fs::write(path, text)
        .with_context(|| format!("writing layout: {}", path.display()))?;
    Ok(())
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_char(c: char) -> String {
    match c {
        '"' => "\\\"".to_string(),
        '\\' => "\\\\".to_string(),
        other => other.to_string(),
    }
}
