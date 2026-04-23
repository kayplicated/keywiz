//! Category aggregation over a [`ScoreResult`]'s hits.
//!
//! Every renderer needs the same per-category rollup (count + cost,
//! optionally the individual hits sorted by contribution). This
//! module owns that logic so text and json don't drift apart.

use drift_core::Hit;
use drift_score::ScoreResult;

/// Per-category rollup of hits. Borrowed view; no copies of `Hit`.
#[derive(Debug)]
pub struct CategoryAgg<'a> {
    pub category: &'static str,
    pub count: usize,
    pub cost: f64,
    /// The hits that contributed to this category, ordered by
    /// descending `|cost|` so callers can take a prefix for "top N".
    pub hits: Vec<&'a Hit>,
}

/// Aggregate `result.hits` by category. Returned vec is sorted by
/// descending `|cost|` — largest-contribution categories first.
pub fn by_category(result: &ScoreResult) -> Vec<CategoryAgg<'_>> {
    use std::collections::BTreeMap;

    let mut map: BTreeMap<&'static str, CategoryAgg<'_>> = BTreeMap::new();
    for hit in &result.hits {
        let entry = map.entry(hit.category).or_insert_with(|| CategoryAgg {
            category: hit.category,
            count: 0,
            cost: 0.0,
            hits: Vec::new(),
        });
        entry.count += 1;
        entry.cost += hit.cost;
        entry.hits.push(hit);
    }

    let mut rows: Vec<CategoryAgg<'_>> = map.into_values().collect();
    for row in &mut rows {
        row.hits.sort_by(|a, b| {
            b.cost
                .abs()
                .partial_cmp(&a.cost.abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    rows.sort_by(|a, b| {
        b.cost
            .abs()
            .partial_cmp(&a.cost.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows
}
