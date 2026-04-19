//! Per-target placement projection.
//!
//! The engine calls the right projector based on which renderer
//! is asking. Terminal wants pos_a/pos_b as integer column/row
//! (schematic, from `r`/`c`); gui wants them as fractional x/y
//! (geometric). Both produce the same [`Placement`] struct shape
//! with different values inside.
//!
//! Stagger-type dispatch lives here rather than in the renderer,
//! so the renderer stays draw-only and the "how do I lay out a
//! row-stag block in terminal" rule has one home.

use crate::engine::placement::Placement;
use crate::keyboard::common::PhysicalKey;
use crate::keyboard::Keyboard;
use crate::mapping::{KeyMapping, Layout};
use crate::stats::Stats;

/// Project the active keyboard for a terminal renderer. Produces
/// placements with `pos_a = c` (column), `pos_b = r` (row), both
/// in integer-valued f32. Terminal-space stagger rules apply here:
///
/// - **col-stag** blocks use `r`/`c` directly (vertical splay is
///   flattened — the whole point of the schematic coordinate).
/// - **row-stag** blocks likewise use `r`/`c` (horizontal splay
///   would require fractional cells, which terminal can't render
///   cleanly; keyboards encode the row shift in `c` if they want
///   it visible).
/// - **free-form** blocks use `r`/`c` (terminal renders them as a
///   flat grid; real xy/rotation is the gui renderer's job).
pub fn project_for_terminal(
    keyboard: &dyn Keyboard,
    layout: &Layout,
    stats: &Stats,
) -> Vec<Placement> {
    keyboard
        .keys()
        .map(|k| Placement {
            id: k.id.clone(),
            pos_a: k.c as f32,
            pos_b: k.r as f32,
            pos_r: 0.0,
            width: 1.0,
            height: 1.0,
            finger: k.finger,
            cluster: k.cluster.clone(),
            label: label_for(k, layout),
            heat: heat_for(k, layout, stats),
        })
        .collect()
}

/// Project for a gui renderer. Produces placements with
/// `pos_a = x`, `pos_b = y`, `pos_r = rotation`, all in
/// key-width units. Gui renderer multiplies by its own pixel
/// scale. Honors real geometry including rotation for
/// fan-shaped thumb clusters.
///
/// Stubbed for now — desktop and webui renderers don't exist
/// yet; the function shape is here so the projector is the single
/// seam when they land.
#[allow(dead_code)]
pub fn project_for_gui(
    keyboard: &dyn Keyboard,
    layout: &Layout,
    stats: &Stats,
) -> Vec<Placement> {
    keyboard
        .keys()
        .map(|k| Placement {
            id: k.id.clone(),
            pos_a: k.x,
            pos_b: k.y,
            pos_r: k.rotation,
            width: k.width,
            height: k.height,
            finger: k.finger,
            cluster: k.cluster.clone(),
            label: label_for(k, layout),
            heat: heat_for(k, layout, stats),
        })
        .collect()
}

/// Resolve a key's layout mapping to a displayable label.
/// Typed chars → the lowercase char. Named actions → the raw
/// action name (renderers format it further if needed).
/// Unmapped → empty string.
fn label_for(key: &PhysicalKey, layout: &Layout) -> String {
    match layout.get(&key.id) {
        Some(KeyMapping::Char { lower, .. }) => lower.to_string(),
        Some(KeyMapping::Named { name }) => name.clone(),
        None => String::new(),
    }
}

/// Heat level for a key, normalized to `0..=1`. Only typed chars
/// have heat (named actions don't participate in drill stats).
fn heat_for(key: &PhysicalKey, layout: &Layout, stats: &Stats) -> Option<f32> {
    let ch = match layout.get(&key.id)? {
        KeyMapping::Char { lower, .. } => *lower,
        KeyMapping::Named { .. } => return None,
    };
    stats.heat_for(ch)
}
