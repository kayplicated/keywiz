//! Heat overlay — paints keys in colors from a heat map.
//!
//! Decoupled from how the heat map is computed: the overlay takes
//! a prebuilt `HashMap<char, f32>` where values are normalized
//! `0.0..=1.0`. That map can be "all-time on this layout", "last
//! 30 days drill-only", "just this session" — the overlay doesn't
//! care. Callers (the engine) pick the filter and rebuild the map
//! when it changes.
//!
//! Default style paints borders — heat reads well as a frame tint
//! without dominating the label the way filled colors would.
//! Users who prefer the old "label-tinted" heat can swap via
//! prefs.

use std::collections::HashMap;

use ratatui::style::Color;

use super::{KeyOverlay, KeyPaint, OverlayContext};
use crate::engine::placement::Placement;
use crate::renderer::terminal::heatmap;
use keywiz_stats::{Event, EventFilter};

/// Which surfaces heat should paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeatStyle {
    pub label: bool,
    pub border: bool,
    pub fill: bool,
}

impl Default for HeatStyle {
    fn default() -> Self {
        Self { label: false, border: true, fill: false }
    }
}

impl HeatStyle {
    #[allow(dead_code)] // staged for prefs integration
    pub const fn labels_only() -> Self {
        Self { label: true, border: false, fill: false }
    }
}

/// Heat overlay — wraps a prebuilt heat map.
pub struct HeatOverlay {
    map: HashMap<char, f32>,
    style: HeatStyle,
}

impl HeatOverlay {
    pub fn new(map: HashMap<char, f32>, style: HeatStyle) -> Self {
        Self { map, style }
    }

    /// Heat for a placement, or `None` if cold (not in the map).
    /// Uses the placement's label (folded lowercase) as the lookup
    /// key — matches the storage convention in the heat view.
    fn heat_for(&self, placement: &Placement) -> Option<f32> {
        // `label` is the raw char for typed keys; match the view's
        // storage convention by lowercasing.
        let ch = placement.label.chars().next()?.to_ascii_lowercase();
        self.map.get(&ch).copied()
    }
}

impl KeyOverlay for HeatOverlay {
    fn paint(&self, placement: &Placement) -> KeyPaint {
        let Some(heat) = self.heat_for(placement) else {
            return KeyPaint::none();
        };
        let color: Color = heatmap::color_for_heat(heat);
        KeyPaint {
            label: self.style.label.then_some(color),
            border: self.style.border.then_some(color),
            fill: self.style.fill.then_some(color),
            modifier: None,
            glyph: None,
        }
    }

    fn name(&self) -> &'static str {
        "heat"
    }

    /// Rebuild the heat map on every keystroke so the overlay
    /// reflects the latest state instantly. A full re-query over
    /// the current layout's events is microseconds — well inside
    /// the render budget — and avoids the complexity of
    /// incrementally updating the integer-step model.
    fn on_event(&mut self, _event: &Event, ctx: &OverlayContext<'_>) {
        let filter = EventFilter {
            layout_hash: Some(ctx.layout_hash.clone()),
            ..Default::default()
        };
        if let Ok(map) = keywiz_stats::views::heat::heat_map(ctx.store, &filter) {
            self.map = map;
        }
    }
}
