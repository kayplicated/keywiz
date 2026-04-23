//! Text renderer for a layout diff.

use std::fmt::Write;

use owo_colors::OwoColorize;

use super::compute::DiffEntry;

/// Render `entries` as a colorized 3-column table. `a_name` and
/// `b_name` label the columns. Empty-bound positions print as `-`.
pub fn render(entries: &[DiffEntry], a_name: &str, b_name: &str) -> String {
    let mut out = String::new();

    let _ = writeln!(
        out,
        "{}",
        format!("Layout diff ({a_name}  vs  {b_name}):").bold()
    );

    if entries.is_empty() {
        let _ = writeln!(out, "  (layouts are identical in their key bindings)");
        return out;
    }

    let _ = writeln!(
        out,
        "  {:<12} {:<10} {:<10}",
        "key".bold(),
        a_name.bright_cyan().bold(),
        b_name.bright_magenta().bold()
    );
    for entry in entries {
        let _ = writeln!(
            out,
            "  {:<12} {:<10} {:<10}",
            entry.key_id,
            char_cell(entry.a).bright_cyan(),
            char_cell(entry.b).bright_magenta()
        );
    }
    out
}

fn char_cell(ch: Option<char>) -> String {
    match ch {
        Some(c) => c.to_string(),
        None => "-".to_string(),
    }
}
