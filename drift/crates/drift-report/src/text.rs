//! Colorized text renderer for score results.
//!
//! Prints per-category rollups and the top few hits per category.
//! Aggregation lives in [`crate::aggregate`] so the json renderer
//! uses the same view.

use std::fmt::Write;

use owo_colors::OwoColorize;

use crate::{Renderer, aggregate};
use drift_score::ScoreResult;

pub struct TextRenderer;

impl Renderer for TextRenderer {
    fn render(&self, result: &ScoreResult) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "{} {}",
            "Layout:".bold(),
            result.layout_name.bright_cyan().bold()
        );
        let _ = writeln!(out, "{}  {}", "Board: ".bold(), result.keyboard_name);
        let _ = writeln!(out, "{} {}", "Corpus:".bold(), result.corpus_name);
        let _ = writeln!(out);

        let cats = aggregate::by_category(result);

        let _ = writeln!(out, "{}", "Category breakdown:".bold());
        for agg in &cats {
            let _ = writeln!(
                out,
                "  {:<22} {:>6} hits   cost {:>10.3}",
                agg.category, agg.count, agg.cost
            );
        }
        let _ = writeln!(out);

        let _ = writeln!(out, "{}", "Top contributions per category:".bold());
        for agg in &cats {
            if agg.cost.abs() < 1e-9 {
                continue;
            }
            let _ = writeln!(out, "  [{}]", agg.category.bright_blue());
            for hit in agg.hits.iter().take(5) {
                let _ = writeln!(out, "    {:<28} {:>10.3}", hit.label, hit.cost);
            }
        }
        let _ = writeln!(out);

        let total = if result.total >= 0.0 {
            format!("{:+.3}", result.total).bright_green().bold().to_string()
        } else {
            format!("{:+.3}", result.total).bright_red().bold().to_string()
        };
        let _ = writeln!(out, "{}  {}", "Overall score:".bold(), total);

        out
    }
}
