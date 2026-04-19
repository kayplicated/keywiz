//! Stagger-type-aware placement for terminal rendering.
//!
//! Each block type gets a placement function that converts its keys
//! to terminal-space integer cells. Terminal is intentionally
//! schematic:
//!
//! - **row-stag**: rows are rigid and rendered flat; fractional `x`
//!   within a row is honored (half-unit shifts = ~3 cells) so ANSI
//!   boards show their row offsets.
//! - **col-stag**: columns are rigid but the vertical splay is
//!   flattened — keys render at `r`/`c` only, ignoring y entirely.
//!   That's the whole point of having `r`/`c`: the terminal gets a
//!   clean schematic, the desktop renderer uses real y.
//! - **free-form**: placed by `r`/`c` directly. Geometric xy is
//!   ignored in terminal (used by desktop for true fan-shape
//!   rendering).

use crate::keyboard::common::PhysicalKey;
use crate::keyboard::{Block, StaggerType};

/// Terminal-space placement of a single key. Integer cells, ready
/// for the draw layer to render at `(col, row)` with size `(w, h)`.
#[derive(Debug, Clone)]
pub struct TerminalPlacement<'a> {
    pub key: &'a PhysicalKey,
    /// Left edge of the key in terminal columns, relative to the
    /// keyboard widget's origin.
    pub col: i32,
    /// Top edge of the key in terminal rows.
    pub row: i32,
    /// Key width in terminal columns.
    pub width: i32,
    /// Key height in terminal rows.
    pub height: i32,
}

/// Terminal cells per unit of grid x (one key-width in columns).
pub const CELL_W: i32 = 5;
/// Terminal lines per unit of grid y (one row-height in lines).
pub const CELL_H: i32 = 3;

/// Place a block's keys in terminal space, dispatching by stagger
/// type. Returns placements with raw (unshifted) cell coordinates;
/// the caller (terminal renderer) re-anchors them into the final
/// widget rectangle.
pub fn place_block<'a>(block: &'a dyn Block) -> Vec<TerminalPlacement<'a>> {
    match block.stagger_type() {
        StaggerType::RowStag => place_rowstag(block),
        StaggerType::ColStag => place_colstag(block),
        StaggerType::FreeForm => place_freeform(block),
    }
}

/// Row-stag: r dictates row, c dictates column baseline, fractional
/// x on top for per-row horizontal offset (ANSI row-stagger).
fn place_rowstag<'a>(block: &'a dyn Block) -> Vec<TerminalPlacement<'a>> {
    block
        .keys()
        .map(|k| {
            // c gives integer column; fractional x on top of that
            // lets row-stag express per-row shifts (top row offset
            // 0.5 relative to home, etc.). We anchor c at 0 meaning
            // "the column position x=0 would map to" — but since our
            // c values are already the schematic column and x tracks
            // real position, just use x for placement on row-stag.
            let col = (k.x * CELL_W as f32).round() as i32;
            let row = k.r * CELL_H;
            TerminalPlacement {
                key: k,
                col,
                row,
                width: (k.width * CELL_W as f32).round().max(3.0) as i32,
                height: CELL_H,
            }
        })
        .collect()
}

/// Col-stag: r and c only. Fractional y offsets from column splay
/// are flattened here — that's the terminal-is-schematic promise.
/// x is still used for horizontal placement because c plus x gives
/// identical results for ortho boards (x == c) but x allows slight
/// widths/positions to flow through if the author sets them.
fn place_colstag<'a>(block: &'a dyn Block) -> Vec<TerminalPlacement<'a>> {
    block
        .keys()
        .map(|k| TerminalPlacement {
            key: k,
            col: k.c * CELL_W,
            row: k.r * CELL_H,
            width: (k.width * CELL_W as f32).round().max(3.0) as i32,
            height: CELL_H,
        })
        .collect()
}

/// Free-form: r and c directly. Geometric xy + rotation is ignored
/// in terminal — the desktop renderer uses those for the true
/// fan-shape of a thumb cluster.
fn place_freeform<'a>(block: &'a dyn Block) -> Vec<TerminalPlacement<'a>> {
    block
        .keys()
        .map(|k| TerminalPlacement {
            key: k,
            col: k.c * CELL_W,
            row: k.r * CELL_H,
            width: (k.width * CELL_W as f32).round().max(3.0) as i32,
            height: CELL_H,
        })
        .collect()
}
