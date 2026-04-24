//! Usage overlay — paints keys by how often they've been pressed.
//!
//! Sibling to [`heat`](super::heat). Where heat answers "which
//! keys are hurting me right now?", usage answers "where do my
//! fingers actually live?" — stable signal that stays informative
//! once fluency has dried up the error heat.
//!
//! Takes a prebuilt `HashMap<char, f32>` from
//! [`keywiz_stats::views::usage`], which log-normalizes counts so
//! rarely-pressed keys stay visible instead of collapsing to zero
//! next to the top-frequency ones.
//!
//! **Drill events are excluded.** Drill is heat-weighted and
//! autoscaling — it hunts weaknesses rather than sampling english,
//! so counting drill presses amplifies whatever was already hot
//! and drowns out the "what does my corpus ask of me" signal the
//! overlay is trying to show. Words + text sessions only. Drill's
//! per-key footprint lives in the error-side heat overlay, which
//! is where that feedback actually belongs.

use std::collections::HashMap;

use ratatui::style::Color;

use super::{KeyOverlay, KeyPaint, OverlayContext};
use crate::engine::placement::Placement;
use crate::renderer::terminal::heatmap;
use keywiz_stats::{Event, EventFilter};

/// Which surfaces usage should paint. Mirrors [`HeatStyle`] —
/// default is border-only so the overlay reads as a frame tint
/// without dominating the label.
///
/// [`HeatStyle`]: super::heat::HeatStyle
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsageStyle {
    pub label: bool,
    pub border: bool,
    pub fill: bool,
}

impl Default for UsageStyle {
    fn default() -> Self {
        Self { label: false, border: true, fill: false }
    }
}

/// Usage overlay — wraps a prebuilt usage map.
pub struct UsageOverlay {
    map: HashMap<char, f32>,
    style: UsageStyle,
}

impl UsageOverlay {
    pub fn new(map: HashMap<char, f32>, style: UsageStyle) -> Self {
        Self { map, style }
    }

    fn usage_for(&self, placement: &Placement) -> Option<f32> {
        let ch = placement.label.chars().next()?.to_ascii_lowercase();
        self.map.get(&ch).copied()
    }
}

impl KeyOverlay for UsageOverlay {
    fn paint(&self, placement: &Placement) -> KeyPaint {
        let Some(usage) = self.usage_for(placement) else {
            return KeyPaint::none();
        };
        let color: Color = heatmap::color_for_usage(usage);
        KeyPaint {
            label: self.style.label.then_some(color),
            border: self.style.border.then_some(color),
            fill: self.style.fill.then_some(color),
            modifier: None,
            glyph: None,
        }
    }

    fn name(&self) -> &'static str {
        "usage"
    }

    /// Rebuild the usage map on every keystroke. Cheaper than it
    /// sounds — `usage_map` is an O(events) scan with integer
    /// increments, well inside the render budget even on large
    /// event streams.
    fn on_event(&mut self, _event: &Event, ctx: &OverlayContext<'_>) {
        let filter = EventFilter {
            layout_hash: Some(ctx.layout_hash.clone()),
            exercise_categories: Some(vec!["words".into(), "text".into()]),
            ..Default::default()
        };
        if let Ok(map) = keywiz_stats::views::usage::usage_map(ctx.store, &filter) {
            self.map = map;
        }
    }
}
