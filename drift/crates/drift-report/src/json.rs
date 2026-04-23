//! JSON renderer — serializes a [`ScoreResult`] with per-category
//! summaries as pretty-printed JSON.
//!
//! The envelope is:
//!   {layout_name, keyboard_name, corpus_name, total, categories,
//!    hits}
//!
//! `categories` is a summary view (name/count/cost) sorted by
//! descending `|cost|`. The raw hits live in `hits`, in pipeline
//! order — consumers who need per-category hits can filter on
//! `hit.category` themselves.

use serde::Serialize;

use crate::{Renderer, aggregate};
use drift_core::Hit;
use drift_score::ScoreResult;

pub struct JsonRenderer;

impl Renderer for JsonRenderer {
    fn render(&self, result: &ScoreResult) -> String {
        let envelope = Envelope::from(result);
        serde_json::to_string_pretty(&envelope)
            .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
    }
}

#[derive(Serialize)]
struct Envelope<'a> {
    layout_name: &'a str,
    keyboard_name: &'a str,
    corpus_name: &'a str,
    total: f64,
    categories: Vec<CategorySummary>,
    hits: &'a [Hit],
}

#[derive(Serialize)]
struct CategorySummary {
    name: &'static str,
    count: usize,
    cost: f64,
}

impl<'a> From<&'a ScoreResult> for Envelope<'a> {
    fn from(result: &'a ScoreResult) -> Self {
        let categories = aggregate::by_category(result)
            .into_iter()
            .map(|agg| CategorySummary {
                name: agg.category,
                count: agg.count,
                cost: agg.cost,
            })
            .collect();
        Envelope {
            layout_name: &result.layout_name,
            keyboard_name: &result.keyboard_name,
            corpus_name: &result.corpus_name,
            total: result.total,
            categories,
            hits: &result.hits,
        }
    }
}
