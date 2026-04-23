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
use crate::keyboard::{Keyboard, StaggerType};
use crate::mapping::{KeyMapping, Layout};

/// Project the active keyboard for a terminal renderer.
///
/// Each block's stagger type drives how the key slots translate to
/// terminal positions:
///
/// - **row-stag**: `pos_a = c + r * 0.5`. Every row shifts half a
///   slot horizontally relative to the row above (ANSI stagger).
///   Home row (`r=0`) is the anchor; rows above shift left, rows
///   below shift right, symmetrically.
/// - **col-stag**: `pos_a = c`. Columns' vertical splay (real `y`)
///   is flattened; keys tile as a clean grid in terminal.
/// - **free-form**: `pos_a = c`. Placed by the schematic slot;
///   geometric xy + rotation is the gui renderer's job.
pub fn project_for_terminal(
    keyboard: &dyn Keyboard,
    layout: &Layout,
) -> Vec<Placement> {
    let mut out: Vec<Placement> = Vec::new();
    for block in keyboard.blocks() {
        let row_shift_factor: f32 = match block.stagger_type() {
            StaggerType::RowStag => 0.5,
            StaggerType::ColStag | StaggerType::FreeForm => 0.0,
        };
        for k in block.keys() {
            let pos_a = k.c as f32 + k.r as f32 * row_shift_factor;
            out.push(Placement {
                id: k.id.clone(),
                pos_a,
                pos_b: k.r as f32,
                pos_r: 0.0,
                width: 1.0,
                height: 1.0,
                finger: k.finger,
                cluster: k.cluster.clone(),
                label: label_for(k, layout),
                typable: is_typable(k, layout),
            });
        }
    }
    out
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
pub fn project_for_gui(keyboard: &dyn Keyboard, layout: &Layout) -> Vec<Placement> {
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
            typable: is_typable(k, layout),
        })
        .collect()
}

/// Whether this key produces a typed character under the active
/// layout. Used by renderers to decide whether a key is a valid
/// highlight target — named actions (shift, tab, enter) aren't.
fn is_typable(key: &PhysicalKey, layout: &Layout) -> bool {
    matches!(layout.get(&key.id), Some(KeyMapping::Char { .. }))
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

