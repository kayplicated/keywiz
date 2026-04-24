//! Overlay system — one decision surface for "how should a key
//! look right now?"
//!
//! An overlay maps a [`Placement`] to a [`KeyPaint`] describing
//! which *surfaces* of the key (label, border, fill, modifier,
//! glyph) should be painted and in what color. `None` on any
//! surface means "don't paint this; leave the renderer's baseline."
//!
//! Rendering flow:
//! 1. Engine holds the active overlay.
//! 2. Renderer calls `overlay.paint(placement)` for each key.
//! 3. Renderer translates `KeyPaint` into its medium — terminal
//!    reads `label/border/fill/modifier`, defers `glyph`; a future
//!    gui renderer reads everything.
//! 4. The current-key highlight stacks *on top* of the overlay —
//!    the user always sees where to type regardless of overlay.
//!
//! ## Why surfaces instead of one color
//!
//! Today's single-color finger overlay paints label + border the
//! same color, which reads as a rainbow on a 30-key board. A
//! user who wants the finger *cue* without the visual weight
//! needs to paint only labels (or only borders). That's a
//! per-surface choice, not a per-color choice. Overlays take a
//! small config struct describing which surfaces to touch.
//!
//! ## Why this shape anticipates gui + theming
//!
//! A future gui renderer draws keycaps with real background
//! colors, real borders, opacity, icons. All of that maps cleanly
//! onto the [`KeyPaint`] fields — no second refactor to enrich
//! overlays when gui lands. A future theme system (CSS-ish,
//! TOML, whatever) is a function from "overlay name + context"
//! to a `KeyPaint` — the same struct today's overlays produce
//! directly. No new abstraction needed; the theme engine slots
//! in as an overlay implementation.

use ratatui::style::{Color, Modifier};

use crate::engine::placement::Placement;
use keywiz_stats::{Event, EventStore, LayoutHash};

pub mod finger;
pub mod heat;
pub mod none;
pub mod usage;

pub use finger::{FingerOverlay, FingerStyle};
pub use heat::{HeatOverlay, HeatStyle};
pub use none::NoneOverlay;
pub use usage::{UsageOverlay, UsageStyle};

/// What an overlay decides to paint on a key.
///
/// Every field is optional: `None` = "this overlay doesn't touch
/// that surface, leave the renderer's baseline." An overlay that
/// only colors labels leaves `border`, `fill`, `modifier`, `glyph`
/// as `None`. The renderer then paints label with the overlay's
/// color and border with whatever its own default is.
///
/// # Surface semantics
///
/// - `label`: the character(s) drawn in the center of the key.
/// - `border`: the box around the key. Terminal uses box chars
///   (`┌─┐`, `╔═╗` for highlight); gui uses CSS border-color.
/// - `fill`: background fill behind the label. Terminal: cell bg;
///   gui: keycap color.
/// - `modifier`: ratatui-style bold/dim/italic/underline. Stacks
///   with colors. Applied wherever the renderer honors modifiers
///   (typically label).
/// - `glyph`: small annotation char rendered in a corner. Terminal
///   ignores unless it fits; gui renders as a tiny icon/badge.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyPaint {
    pub label: Option<Color>,
    pub border: Option<Color>,
    pub fill: Option<Color>,
    pub modifier: Option<Modifier>,
    pub glyph: Option<char>,
}

impl KeyPaint {
    /// An empty paint — nothing to draw, renderer uses its defaults.
    pub const fn none() -> Self {
        Self {
            label: None,
            border: None,
            fill: None,
            modifier: None,
            glyph: None,
        }
    }
}

/// What a key looks like under the currently-active overlay.
pub trait KeyOverlay {
    /// Decide how to paint one key.
    fn paint(&self, placement: &Placement) -> KeyPaint;

    /// Stable identifier, e.g. `"none"`, `"finger"`, `"heat"`.
    /// Used for prefs persistence and F2-cycle display.
    fn name(&self) -> &'static str;

    /// Called after every keystroke the engine processes. Overlays
    /// that cache per-event derived data (heat maps, recency,
    /// accuracy trails) use this to stay current without the
    /// engine having to know which overlays care. Default no-op
    /// so overlays without per-event state (none, finger) don't
    /// have to implement anything.
    fn on_event(&mut self, _event: &Event, _ctx: &OverlayContext<'_>) {}
}

/// Everything an overlay might want to recompute itself after a
/// keystroke. Grows as new overlays land — current consumers can
/// ignore fields they don't need. Keep it reference-only so the
/// engine stays the single owner.
pub struct OverlayContext<'a> {
    /// The event store, for overlays that need to query broader
    /// slices than just "the event that fired" (e.g. heat: rebuild
    /// the full map for the current layout).
    pub store: &'a dyn EventStore,
    /// Content hash of the currently-active layout. Overlays that
    /// scope their data per layout iteration filter on this.
    pub layout_hash: &'a LayoutHash,
}
