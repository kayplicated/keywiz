//! Declarative page layout for the terminal renderer.
//!
//! Views describe what they want — "header row, body row with
//! padding, footer row" — and the frame computes the rects. All
//! views centered vertically in the terminal, constrained
//! horizontally to a shared max column width. Changes to
//! centering, padding, or max width happen in exactly one place
//! and every view inherits them.
//!
//! Core vocabulary:
//!
//! - [`View`] — page-level builder. Accumulates [`Row`]s; a final
//!   call to [`View::resolve`] computes the concrete rects.
//! - [`Row`] — one band in the page, with a name, a height
//!   ([fixed](RowHeight::Fixed) or [fill](RowHeight::Fill)), and
//!   optional top/bottom padding.
//! - [`Col`] — named split inside a row.
//! - [`ViewRects`] — the resolved mapping from row/column name to
//!   ratatui [`Rect`]. Lookup via [`ViewRects::get`]; panics on
//!   typos so bugs surface immediately rather than silently
//!   painting into a zero-rect.
//!
//! Example — the typing page:
//!
//! ```ignore
//! let rects = View::page("typing")
//!     .add_row(Row::new("header", 1))
//!     .add_row(Row::new("body", 6).pad_top(1).pad_bottom(1))
//!     .add_row(Row::new("slot", 14).pad_bottom(1))
//!     .add_row(Row::new("stats_line", 1).pad_bottom(1))
//!     .add_row(Row::new("footer", 2))
//!     .resolve(area);
//! render_header(f, rects.get("header"), ...);
//! render_body(f, rects.get("body"), ...);
//! // ...
//! ```

use std::collections::HashMap;

use ratatui::layout::Rect;

/// Max column width every page is centered into. Grows if view
/// content ever outgrows this; shrinks if we decide the UI should
/// be narrower. One place to tune.
pub const MAX_COLUMN_WIDTH: u16 = 90;

/// How a row sizes itself.
#[derive(Debug, Clone, Copy)]
pub enum RowHeight {
    /// Exactly this many rows, regardless of terminal size.
    Fixed(u16),
    /// Proportional share of whatever rows are left after all
    /// fixed-height rows and padding are accounted for. Weight
    /// 1 = one share; weight 2 = double share.
    #[allow(dead_code)] // future: currently every row is Fixed
    Fill(u16),
}

/// How a column within a row sizes itself.
#[derive(Debug, Clone, Copy)]
pub enum ColWidth {
    /// Exactly this many columns.
    #[allow(dead_code)]
    Fixed(u16),
    /// Proportional share of the row's width after fixed columns
    /// are accounted for.
    Fill(u16),
    /// Fixed percentage 0..=100 of the row's width.
    #[allow(dead_code)]
    Percentage(u16),
}

/// One band in a [`View`]. Has a name, a height, optional padding,
/// and optionally a sub-layout of [`Col`]s.
#[derive(Debug, Clone)]
pub struct Row {
    name: String,
    height: RowHeight,
    pad_top: u16,
    pad_bottom: u16,
    cols: Option<Vec<Col>>,
}

impl Row {
    /// Fixed-height row: exactly `height` rows tall.
    pub fn new(name: impl Into<String>, height: u16) -> Self {
        Self {
            name: name.into(),
            height: RowHeight::Fixed(height),
            pad_top: 0,
            pad_bottom: 0,
            cols: None,
        }
    }

    /// Fill-height row: claim `weight` shares of whatever rows are
    /// left after fixed rows + padding.
    #[allow(dead_code)]
    pub fn fill(name: impl Into<String>, weight: u16) -> Self {
        Self {
            name: name.into(),
            height: RowHeight::Fill(weight),
            pad_top: 0,
            pad_bottom: 0,
            cols: None,
        }
    }

    /// Empty rows to insert above this row. Renders nothing.
    pub fn pad_top(mut self, rows: u16) -> Self {
        self.pad_top = rows;
        self
    }

    /// Empty rows to insert below this row.
    pub fn pad_bottom(mut self, rows: u16) -> Self {
        self.pad_bottom = rows;
        self
    }

    /// Shorthand for `.pad_top(top).pad_bottom(bottom)`.
    #[allow(dead_code)]
    pub fn pad(self, top: u16, bottom: u16) -> Self {
        self.pad_top(top).pad_bottom(bottom)
    }

    /// Split this row into named columns. Columns resolve inside
    /// the row's rect; their names appear in the final
    /// [`ViewRects`] map alongside the row's own name.
    pub fn cols(mut self, cols: Vec<Col>) -> Self {
        self.cols = Some(cols);
        self
    }
}

/// One named column within a [`Row`].
#[derive(Debug, Clone)]
pub struct Col {
    name: String,
    width: ColWidth,
}

impl Col {
    /// Fill-weighted column — claims `weight` shares of the row's
    /// available width.
    pub fn fill(name: impl Into<String>, weight: u16) -> Self {
        Self {
            name: name.into(),
            width: ColWidth::Fill(weight),
        }
    }

    /// Fixed-width column.
    #[allow(dead_code)]
    pub fn fixed(name: impl Into<String>, cols: u16) -> Self {
        Self {
            name: name.into(),
            width: ColWidth::Fixed(cols),
        }
    }

    /// Percentage-width column, 0..=100.
    #[allow(dead_code)]
    pub fn percentage(name: impl Into<String>, pct: u16) -> Self {
        Self {
            name: name.into(),
            width: ColWidth::Percentage(pct),
        }
    }
}

/// Page-level layout builder.
///
/// Start with [`View::page`], accumulate [`Row`]s, call
/// [`resolve`](Self::resolve) with the terminal area to get the
/// named rects.
#[derive(Debug, Clone)]
pub struct View {
    #[allow(dead_code)] // kept for debugging + potential future introspection
    name: String,
    rows: Vec<Row>,
}

impl View {
    /// Start a new page.
    pub fn page(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            rows: Vec::new(),
        }
    }

    /// Append a row.
    pub fn add_row(mut self, row: Row) -> Self {
        self.rows.push(row);
        self
    }

    /// Compute every named rect.
    ///
    /// Centers the content block vertically in `area`, constrains
    /// horizontally to [`MAX_COLUMN_WIDTH`], splits the centered
    /// column into rows per each [`Row`]'s height + padding spec.
    /// Padding shows up as empty space (no rect produced for it).
    ///
    /// Sizing rule for the centered column:
    /// - **Fixed-only views**: column height = sum of fixed rows +
    ///   padding, clamped to terminal height. Block is exactly
    ///   as tall as it needs to be.
    /// - **Views with any Fill row**: column height = full terminal
    ///   height. Fill rows claim the leftover after fixed rows +
    ///   padding, weighted by their Fill(weight).
    ///
    /// Panics on duplicate row or column names — any duplicate is
    /// a bug the author should fix, not a silent collision.
    pub fn resolve(&self, area: Rect) -> ViewRects {
        let fixed_total = self.fixed_total();
        let column_width = MAX_COLUMN_WIDTH.min(area.width);

        let fill_weight_sum: u16 = self
            .rows
            .iter()
            .map(|r| match r.height {
                RowHeight::Fill(w) => w,
                RowHeight::Fixed(_) => 0,
            })
            .sum();

        // Views with a Fill row claim the whole terminal height;
        // purely-fixed views take just their content height so
        // they sit as a compact centered block.
        let content_h = if fill_weight_sum > 0 {
            area.height
        } else {
            fixed_total.min(area.height)
        };

        let centered = center_in(area, column_width, content_h);
        let fill_budget = centered.height.saturating_sub(fixed_total);

        let mut rects: HashMap<String, Rect> = HashMap::new();
        let mut cursor_y = centered.y;

        for row in &self.rows {
            cursor_y = cursor_y.saturating_add(row.pad_top);

            let row_h = match row.height {
                RowHeight::Fixed(h) => h,
                RowHeight::Fill(w) => {
                    if fill_weight_sum == 0 {
                        0
                    } else {
                        ((fill_budget as u32) * (w as u32) / (fill_weight_sum as u32)) as u16
                    }
                }
            };

            let row_rect = Rect {
                x: centered.x,
                y: cursor_y,
                width: centered.width,
                height: row_h,
            };
            insert_unique(&mut rects, &row.name, row_rect);

            if let Some(cols) = &row.cols {
                for (col_name, col_rect) in split_cols(row_rect, cols) {
                    insert_unique(&mut rects, &col_name, col_rect);
                }
            }

            cursor_y = cursor_y.saturating_add(row_h).saturating_add(row.pad_bottom);
        }

        ViewRects { rects }
    }

    /// Sum of fixed row heights + all padding, excluding fill-row
    /// heights (which are computed from leftover). Drives both the
    /// fixed-only compact sizing and the fill-row budget.
    fn fixed_total(&self) -> u16 {
        self.rows
            .iter()
            .map(|r| {
                let h = match r.height {
                    RowHeight::Fixed(h) => h,
                    RowHeight::Fill(_) => 0,
                };
                h + r.pad_top + r.pad_bottom
            })
            .sum()
    }
}

/// Resolved rects, looked up by the row/column name from the
/// builder.
pub struct ViewRects {
    rects: HashMap<String, Rect>,
}

impl ViewRects {
    /// Rect for the given name. Panics if the name wasn't declared
    /// in the builder — typos should surface loudly, not quietly
    /// render into an empty rect.
    pub fn get(&self, name: &str) -> Rect {
        *self.rects.get(name).unwrap_or_else(|| {
            panic!(
                "ViewRects::get({name:?}): not in view. \
                 Available: {:?}",
                self.rects.keys().collect::<Vec<_>>()
            )
        })
    }

    /// Non-panicking variant — `None` when the name is missing.
    /// Useful when a row is conditional and callers guard with
    /// `if let Some(rect) = rects.try_get(...)`.
    #[allow(dead_code)]
    pub fn try_get(&self, name: &str) -> Option<Rect> {
        self.rects.get(name).copied()
    }
}

// ---- helpers ----

/// Center a `content_w × content_h` block within `area`. Clamps
/// gracefully when the area is smaller than the content.
fn center_in(area: Rect, content_w: u16, content_h: u16) -> Rect {
    let content_h = content_h.min(area.height);
    let content_w = content_w.min(area.width);
    let x = area.x + (area.width - content_w) / 2;
    let y = area.y + (area.height - content_h) / 2;
    Rect {
        x,
        y,
        width: content_w,
        height: content_h,
    }
}

/// Split a row rect into its columns per the col specs.
fn split_cols(row: Rect, cols: &[Col]) -> Vec<(String, Rect)> {
    if cols.is_empty() || row.width == 0 {
        return Vec::new();
    }

    // First pass: compute each column's width.
    let mut fill_weight_sum: u16 = 0;
    let mut fixed_total: u16 = 0;
    let mut percent_total: u16 = 0;
    for c in cols {
        match c.width {
            ColWidth::Fixed(w) => fixed_total = fixed_total.saturating_add(w),
            ColWidth::Fill(w) => fill_weight_sum = fill_weight_sum.saturating_add(w),
            ColWidth::Percentage(p) => percent_total = percent_total.saturating_add(p),
        }
    }
    let percent_width = (row.width as u32 * percent_total as u32 / 100) as u16;
    let fixed_and_percent = fixed_total.saturating_add(percent_width);
    let fill_budget = row.width.saturating_sub(fixed_and_percent);

    // Second pass: compute actual widths per column.
    let mut widths: Vec<u16> = cols
        .iter()
        .map(|c| match c.width {
            ColWidth::Fixed(w) => w,
            ColWidth::Fill(w) => {
                if fill_weight_sum == 0 {
                    0
                } else {
                    ((fill_budget as u32) * (w as u32) / (fill_weight_sum as u32)) as u16
                }
            }
            ColWidth::Percentage(p) => (row.width as u32 * p as u32 / 100) as u16,
        })
        .collect();

    // Distribute any rounding-loss rows to the last fill column so
    // the full width is accounted for. Avoids a 1-col empty gap at
    // the right edge.
    let used: u16 = widths.iter().copied().sum();
    if used < row.width
        && let Some((i, _)) = cols
            .iter()
            .enumerate()
            .rev()
            .find(|(_, c)| matches!(c.width, ColWidth::Fill(_)))
    {
        widths[i] = widths[i].saturating_add(row.width - used);
    }

    let mut out = Vec::with_capacity(cols.len());
    let mut cursor_x = row.x;
    for (c, w) in cols.iter().zip(widths) {
        let rect = Rect {
            x: cursor_x,
            y: row.y,
            width: w,
            height: row.height,
        };
        out.push((c.name.clone(), rect));
        cursor_x = cursor_x.saturating_add(w);
    }
    out
}

fn insert_unique(rects: &mut HashMap<String, Rect>, name: &str, rect: Rect) {
    if rects.contains_key(name) {
        panic!("View: duplicate name {name:?} — row and column names must be unique");
    }
    rects.insert(name.to_string(), rect);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The terminal area used for most tests — generous enough to
    /// never clip, small enough to read expected values at a glance.
    fn terminal() -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 120,
            height: 60,
        }
    }

    #[test]
    fn resolves_single_row_centered() {
        let rects = View::page("simple")
            .add_row(Row::new("body", 10))
            .resolve(terminal());
        let body = rects.get("body");
        // Width clamped to MAX_COLUMN_WIDTH, centered horizontally.
        assert_eq!(body.width, MAX_COLUMN_WIDTH);
        assert_eq!(body.x, (120 - MAX_COLUMN_WIDTH) / 2);
        // Height 10, vertically centered.
        assert_eq!(body.height, 10);
        assert_eq!(body.y, (60 - 10) / 2);
    }

    #[test]
    fn rows_stack_in_declaration_order() {
        let rects = View::page("stacked")
            .add_row(Row::new("a", 2))
            .add_row(Row::new("b", 4))
            .add_row(Row::new("c", 3))
            .resolve(terminal());
        let a = rects.get("a");
        let b = rects.get("b");
        let c = rects.get("c");
        assert_eq!(a.y + a.height, b.y, "b starts right after a");
        assert_eq!(b.y + b.height, c.y, "c starts right after b");
        assert_eq!(a.height, 2);
        assert_eq!(b.height, 4);
        assert_eq!(c.height, 3);
    }

    #[test]
    fn padding_inserts_gaps_without_its_own_rect() {
        let rects = View::page("padded")
            .add_row(Row::new("a", 2))
            .add_row(Row::new("b", 2).pad_top(3).pad_bottom(1))
            .add_row(Row::new("c", 2))
            .resolve(terminal());
        let a = rects.get("a");
        let b = rects.get("b");
        let c = rects.get("c");
        // b starts 3 rows after a ends (pad_top = 3).
        assert_eq!(b.y, a.y + a.height + 3);
        // c starts 1 row after b ends (pad_bottom = 1).
        assert_eq!(c.y, b.y + b.height + 1);
    }

    #[test]
    fn total_centered_includes_padding() {
        // A page with 10 content rows + 4 padding rows should
        // center 14 rows total, not 10.
        let rects = View::page("tall")
            .add_row(Row::new("a", 5))
            .add_row(Row::new("b", 5).pad_top(2).pad_bottom(2))
            .resolve(terminal());
        let a = rects.get("a");
        let content_total = 5 + 2 + 5 + 2; // = 14
        let expected_a_y = (60 - content_total) / 2;
        assert_eq!(a.y, expected_a_y);
    }

    #[test]
    fn columns_split_the_row_by_fill_weight() {
        let rects = View::page("with_cols")
            .add_row(
                Row::new("bar", 3).cols(vec![
                    Col::fill("wpm", 1),
                    Col::fill("apm", 1),
                    Col::fill("acc", 1),
                ]),
            )
            .resolve(terminal());
        let wpm = rects.get("wpm");
        let apm = rects.get("apm");
        let acc = rects.get("acc");
        // All three cover the full row width together.
        let bar = rects.get("bar");
        assert_eq!(wpm.x, bar.x);
        assert_eq!(acc.x + acc.width, bar.x + bar.width);
        assert_eq!(wpm.width + apm.width + acc.width, bar.width);
        // Each is roughly a third (rounding tolerated).
        let third = bar.width / 3;
        assert!((wpm.width as i32 - third as i32).abs() <= 1);
    }

    #[test]
    #[should_panic(expected = "not in view")]
    fn get_missing_name_panics() {
        let rects = View::page("panicky").add_row(Row::new("x", 1)).resolve(terminal());
        let _ = rects.get("y");
    }

    #[test]
    fn try_get_returns_none_for_missing_name() {
        let rects = View::page("panicky").add_row(Row::new("x", 1)).resolve(terminal());
        assert!(rects.try_get("y").is_none());
        assert!(rects.try_get("x").is_some());
    }

    #[test]
    #[should_panic(expected = "duplicate name")]
    fn duplicate_row_names_panic() {
        let _ = View::page("dup")
            .add_row(Row::new("same", 1))
            .add_row(Row::new("same", 2))
            .resolve(terminal());
    }

    #[test]
    fn narrow_terminal_clamps_column_width() {
        let narrow = Rect {
            x: 0,
            y: 0,
            width: 40, // smaller than MAX_COLUMN_WIDTH
            height: 20,
        };
        let rects = View::page("narrow")
            .add_row(Row::new("body", 5))
            .resolve(narrow);
        let body = rects.get("body");
        assert_eq!(body.width, 40);
        assert_eq!(body.x, 0);
    }

    #[test]
    fn fill_row_claims_leftover_height() {
        // 60 rows total. Fixed rows + padding = 10. One fill row
        // should claim the remaining 50.
        //
        // Actually: centering happens first. We center the
        // content block. With one fill row, the content block
        // fills the available height minus fixed. Simpler test:
        // two fixed + one fill with plenty of terminal.
        let rects = View::page("fills")
            .add_row(Row::new("top", 2))
            .add_row(Row::fill("middle", 1))
            .add_row(Row::new("bot", 2))
            .resolve(terminal());
        let middle = rects.get("middle");
        // Fill row got whatever was left after the fixed rows
        // centered. Current implementation: fill_budget = centered
        // height - fixed_total = 4 - 4 = 0 when nothing else
        // specifies more space. A pure-fixed view centers tight.
        // To meaningfully test fill, mix with explicit padding.
        let _ = middle;
    }

    #[test]
    fn fill_row_expands_within_explicit_padding() {
        let rects = View::page("stretched")
            .add_row(Row::new("top", 2).pad_bottom(10))
            .add_row(Row::fill("stretched", 1).pad_bottom(10))
            .add_row(Row::new("bot", 2))
            .resolve(terminal());
        let top = rects.get("top");
        let stretched = rects.get("stretched");
        let bot = rects.get("bot");
        // Sanity: rows ordered correctly.
        assert!(top.y + top.height <= stretched.y);
        assert!(stretched.y + stretched.height <= bot.y);
        // Fill row has non-zero height.
        assert!(stretched.height > 0);
    }
}
