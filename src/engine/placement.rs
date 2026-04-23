//! Placement and DisplayState — the shapes the engine hands to
//! renderers.
//!
//! A [`Placement`] is a single key ready to draw. Its `pos_a`,
//! `pos_b`, and `pos_r` fields carry coordinates in whatever units
//! the target renderer expects; the engine's per-target projector
//! fills them appropriately. Renderers are target-aware (terminal
//! knows pos_a is a column, gui knows pos_a is an x) and never
//! inspect the keyboard, layout, or stats directly.
//!
//! A [`DisplayState`] carries everything else a frame needs — the
//! indicator strings, the broken-selection markers, the display
//! toggles, and exercise-specific state fields. Renderers read it
//! field by field; the engine populates whatever the active
//! exercise makes available.

use crate::keyboard::common::Finger;

/// A single drawable key.
///
/// The `pos_a` / `pos_b` / `pos_r` naming is intentional: each
/// renderer interprets the values in its own coordinate system
/// (terminal reads pos_a as column and pos_b as row, ignores pos_r;
/// gui reads them as x/y with rotation). The Placement itself does
/// not commit to an interpretation.
#[derive(Debug, Clone)]
pub struct Placement {
    /// Physical key id — stable across frames. Staged for gui
    /// click/touch events and future highlight-by-id paths; the
    /// terminal renderer matches by label char today.
    #[allow(dead_code)]
    pub id: String,
    /// First positional coordinate. Terminal: column in key-grid
    /// units (integer-valued f32). Gui: x in key-width units.
    pub pos_a: f32,
    /// Second positional coordinate. Terminal: row. Gui: y.
    pub pos_b: f32,
    /// Rotation in degrees. Staged for gui — the terminal ignores
    /// rotation.
    #[allow(dead_code)]
    pub pos_r: f32,
    /// Key cap width in the target's native units.
    pub width: f32,
    /// Key cap height in the target's native units.
    pub height: f32,
    pub finger: Finger,
    /// Cluster name (e.g. `"main"`, `"left_thumb"`). Staged for
    /// per-cluster theming; no renderer reads it yet.
    #[allow(dead_code)]
    pub cluster: String,
    /// Pre-formatted label text to display on the key cap. Empty
    /// when the layout doesn't map this key ("dead" / unmapped).
    pub label: String,
    /// `true` when the layout maps this key to a typed character
    /// (KeyMapping::Char). `false` for named actions (shift, tab,
    /// enter) and for unmapped keys. Renderers use this to decide
    /// whether a key is highlightable as a typing target.
    pub typable: bool,
}

/// Everything a renderer needs to paint a frame beyond the
/// placements themselves: indicator strings, toggles, exercise
/// state. Fields are optional per exercise — only the fields the
/// active exercise populates are `Some`.
#[derive(Debug, Clone, Default)]
pub struct DisplayState {
    // ---- always populated ----
    pub keyboard_short: String,
    pub layout_short: String,
    pub exercise_short: String,
    /// `(current_index, total_count)` for the exercise's instance
    /// axis (words lengths, text passages). `(0, 0)` when the
    /// category has no instance axis (drill). Indices are 1-based
    /// for display.
    pub exercise_instance: (usize, usize),
    /// Human label for the current instance, e.g. `"50"`,
    /// `"Endless"`, `"The Commit"`. `None` when there's no
    /// instance axis.
    pub exercise_instance_label: Option<String>,
    pub broken_keyboard: Option<BrokenDisplay>,
    pub broken_layout: Option<BrokenDisplay>,
    /// Whether the keyboard slot (whatever's currently in it —
    /// keyboard, inline stats, future) is showing. Tab toggles.
    pub slot_visible: bool,
    /// Which content is in the keyboard slot. `"keyboard"` or
    /// `"inline_stats"`. Renderer picks the painter by string so
    /// adding a new slot doesn't ripple through types.
    pub slot: &'static str,
    /// F1 help page. When `true`, renderer replaces everything
    /// with the keybind reference.
    pub help_page_visible: bool,
    /// F4 stats page. When `true`, renderer replaces everything
    /// with the full stats page.
    pub stats_page_visible: bool,
    /// F5 layout-iterations page. When `true`, renderer replaces
    /// everything with the iterations view. Orthogonal to F4 —
    /// answers "how is the layout performing" vs F4's "how am I
    /// typing."
    pub layout_page_visible: bool,
    /// Which stats view the F4 page is showing. `"current_session"`
    /// or `"session_history"`. Same stringly-typed pattern as
    /// `slot`.
    pub stats_view: &'static str,
    /// Name of the currently-active overlay (see
    /// `renderer::overlay::KeyOverlay::name`). `"none"` when no
    /// overlay is painting. Renderers can surface this in the
    /// footer so the user sees which overlay is active.
    pub overlay_name: &'static str,
    /// Character the user should press next (drill's current, the
    /// next char of the current word, etc.). Renderers use it to
    /// highlight the corresponding key.
    pub highlight_char: Option<char>,
    pub session_accuracy: f64,
    /// Net WPM for the active session (correct chars / 5 / minutes
    /// of active typing time). Zero on an empty session.
    pub session_wpm: f64,
    /// Actions (keystrokes) per minute of active typing time. Raw
    /// hand-speed signal; WPM is APM / 5.
    pub session_apm: f64,
    pub session_total_correct: u64,
    pub session_total_wrong: u64,

    // ---- drill ----
    pub drill_current_char: Option<char>,
    pub drill_level: Option<String>,
    pub drill_streak: Option<u32>,

    // ---- words ----
    pub words: Option<WordsDisplay>,

    // ---- text ----
    pub text: Option<TextDisplay>,
}

#[derive(Debug, Clone)]
pub struct BrokenDisplay {
    pub name: String,
    pub reason: String,
}

/// State specific to a words-style exercise. Renderer uses this
/// to draw the scrolling word display with cursor.
#[derive(Debug, Clone)]
pub struct WordsDisplay {
    /// Flat list of characters across all words, with per-char
    /// status so the renderer can color them.
    pub chars: Vec<WordsChar>,
    /// Index into `chars` where the cursor sits.
    pub cursor: usize,
    /// Number of completed words (for the header counter).
    pub word_index: usize,
    pub is_finished: bool,
}

#[derive(Debug, Clone)]
pub struct WordsChar {
    pub ch: char,
    pub status: WordsCharStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordsCharStatus {
    /// Already typed correctly.
    Done,
    /// The character under the cursor — "type this next."
    Cursor,
    /// Upcoming in the current word.
    Pending,
    /// Separator between words (space shown as `·` dot).
    Separator,
    /// Already completed word.
    CompletedWord,
}

/// State specific to a passage-typing exercise. Passage position
/// (n/m) is on the shared indicator fields; this struct only
/// carries what's specific to the body render.
#[derive(Debug, Clone)]
pub struct TextDisplay {
    pub title: String,
    /// Full body text; renderer decides how to window it.
    pub body: String,
    /// Character cursor within `body`.
    pub cursor: usize,
    pub is_finished: bool,
}
