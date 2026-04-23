//! The runtime Engine — owns the active keyboard, layout, exercise,
//! stats, and display state. One coordinator the rest of the app
//! talks to.

use std::path::{Path, PathBuf};

use crate::engine::catalog::{
    generic_layout_names, keyboard_path, list_json_stems, load_layout_resolved, next_in,
    pick_first_loadable, prev_in, KEYBOARDS_DIR, LAYOUTS_DIR,
};
use crate::engine::placement::{BrokenDisplay, DisplayState, Placement};
use crate::engine::projector::project_for_terminal;
use crate::engine::stats_filter::{Combo, Granularity, StatsFilter};
use crate::engine::translate::{self, Translator};
use crate::exercise::{catalog as exercise_catalog, Exercise};
use crate::keyboard::{self, Keyboard};
use crate::mapping::Layout;
use crate::renderer::overlay::{
    FingerOverlay, FingerStyle, HeatOverlay, HeatStyle, KeyOverlay, NoneOverlay,
};
use crate::stats_adapter::{keyboard_snapshot, layout_snapshot};

use std::collections::HashMap;

/// Relative path under the user's data directory for the event
/// store. Kept as a constant so tests / CLI overrides have one
/// place to point at.
const EVENTS_STORE_RELPATH: &str = "keywiz/stats.sqlite";

#[derive(Debug, Clone)]
pub struct LayoutChange {
    pub from: String,
}

#[derive(Debug, Clone)]
pub struct BrokenSelection {
    pub name: String,
    pub reason: String,
}

impl From<&BrokenSelection> for BrokenDisplay {
    fn from(b: &BrokenSelection) -> Self {
        BrokenDisplay {
            name: b.name.clone(),
            reason: b.reason.clone(),
        }
    }
}

#[derive(Debug)]
pub enum EngineError {
    UnknownKeyboard(String),
    UnknownLayout(String),
    Broken { name: String, message: String },
    Load(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::UnknownKeyboard(n) => write!(f, "unknown keyboard: {n}"),
            EngineError::UnknownLayout(n) => write!(f, "unknown layout: {n}"),
            EngineError::Broken { name, message } => write!(f, "{name}: {message}"),
            EngineError::Load(msg) => write!(f, "{msg}"),
        }
    }
}

/// Outcome of processing one input char. Staged for metrics / stats
/// hooks — today main.rs discards the return value, but the fields
/// are the obvious seam for per-hit side effects (sound, haptics,
/// session timers, end-of-exercise dispatch).
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct KeystrokeResult {
    pub hit: bool,
    /// Whether the active exercise is now done.
    pub exercise_done: bool,
}

pub struct Engine {
    keyboards_dir: PathBuf,
    layouts_dir: PathBuf,
    keyboards: Vec<String>,
    layouts: Vec<String>,
    current_keyboard: String,
    current_layout: String,
    /// Active exercise category (`drill`, `words`, `text`).
    current_category: String,
    /// Active instance index within the current category.
    current_instance: usize,
    /// Per-category memory of the last instance visited. Lets users
    /// cycle away from text passage 7 to drill and back without
    /// losing their place.
    instance_memory: std::collections::HashMap<String, usize>,
    keyboard: Box<dyn Keyboard>,
    layout: Layout,
    exercise: Box<dyn Exercise>,
    translator: Translator,
    from_layout: Option<String>,
    /// Event-stream stats. `None` when the SQLite store could not
    /// be opened (permissions, bad path, corrupted db); keywiz
    /// stays usable without it, just without recording stats.
    events: Option<keywiz_stats::Stats>,
    /// True between session boundaries (layout/keyboard/exercise
    /// switch) when `events` has no active session yet. The first
    /// [`process_input`] after a boundary lazily starts one, so
    /// empty-session rows don't pile up for launches where the
    /// user switches context without typing.
    events_session_pending: bool,
    broken_keyboard: Option<BrokenSelection>,
    broken_layout: Option<BrokenSelection>,
    /// What's shown in the area below the typing body.
    slot: KeyboardSlot,
    /// Whether that slot is visible. Tab toggles this; F4 cycles
    /// `slot`. Hidden means "show typing body only."
    slot_visible: bool,
    /// F1 help page modal state. Typing pauses; the help page
    /// replaces every other surface.
    help_page_visible: bool,
    /// F4 full-screen stats modal state. When visible, typing is
    /// paused and the stats page replaces every other surface.
    stats_page_visible: bool,
    /// F5 layout-iterations modal state. Orthogonal to F4 — F4
    /// answers "how am I typing" (performance across time), F5
    /// answers "how is the layout performing" (performance
    /// across its content-hash iterations).
    layout_page_visible: bool,
    /// Which stats page the F4 modal is showing. Cycled with
    /// Ctrl+←/→ when the modal is open.
    active_stats_view: StatsView,
    /// Active filter scope for stats-page queries (layout /
    /// keyboard / granularity / offset). Cycled with Ctrl+±/Alt+±
    /// when the modal is open — same keys as typing-view cycling,
    /// context-appropriate meaning.
    stats_filter: StatsFilter,
    /// Currently-active overlay. Cycled by F2 through
    /// none → finger → heat → none. Owned here so the engine can
    /// rebuild the heat map when it decides the underlying data
    /// changed (layout switch, new heat-relevant events).
    active_overlay: Box<dyn KeyOverlay>,
    /// Whether the flash-on-keypress layer is on. Orthogonal to
    /// [`Self::active_overlay`] — flash stacks on whatever overlay
    /// is painting. Toggled by Shift+Tab.
    flash_enabled: bool,
    /// Most recent keystroke (lowercased char + wall-clock time).
    /// Renderer reads this to paint a fading flash; `None` when no
    /// keystroke has happened yet, or when flash is disabled the
    /// field is still populated but the renderer ignores it.
    last_flash: Option<FlashKeystroke>,
}

/// One keystroke's worth of flash input. The renderer mixes
/// `at`'s age against a fade duration to pick a tint intensity.
#[derive(Debug, Clone, Copy)]
pub struct FlashKeystroke {
    pub char: char,
    pub at: std::time::Instant,
}

/// What occupies the keyboard area below the typing body.
/// Cycled by F4; independent of whether the slot is visible (Tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardSlot {
    /// The visual keyboard (default).
    Keyboard,
    /// A compact inline stats strip — live WPM/APM/accuracy for
    /// users who've memorized their layout and don't need the
    /// keyboard picture.
    InlineStats,
}

impl KeyboardSlot {
    /// F4 cycle order.
    fn cycle_next(self) -> Self {
        match self {
            Self::Keyboard => Self::InlineStats,
            Self::InlineStats => Self::Keyboard,
        }
    }
}

/// Which page the F4 stats modal is showing. Cycled with Ctrl+←/→
/// inside the modal (the same keys that cycle layout during typing
/// — rebound by context).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsView {
    /// P1 — dense overview: numbers + rhythm + peak + worst bigrams
    /// + worst keys + APM sparkline for the active scope.
    Overview,
    /// P2 — progression: series + sparkline over buckets at the
    /// chosen granularity. Stubbed pending Phase 2.
    Progression,
    /// P3 — layout × you: finger load + heat over time + (later)
    /// drift cross-reference. Stubbed pending Phase 3.
    LayoutView,
}

impl StatsView {
    fn cycle_next(self) -> Self {
        match self {
            Self::Overview => Self::Progression,
            Self::Progression => Self::LayoutView,
            Self::LayoutView => Self::Overview,
        }
    }

    fn cycle_prev(self) -> Self {
        match self {
            Self::Overview => Self::LayoutView,
            Self::Progression => Self::Overview,
            Self::LayoutView => Self::Progression,
        }
    }
}

impl Engine {
    pub fn new(from_layout: Option<String>) -> Result<Self, EngineError> {
        Self::with_dirs(
            Path::new(KEYBOARDS_DIR),
            Path::new(LAYOUTS_DIR),
            from_layout,
        )
    }

    pub fn with_dirs(
        keyboards_dir: &Path,
        layouts_dir: &Path,
        from_layout: Option<String>,
    ) -> Result<Self, EngineError> {
        let keyboards = list_json_stems(keyboards_dir);
        let layouts = generic_layout_names(layouts_dir, &keyboards);

        let (initial_keyboard, keyboard) =
            pick_first_loadable(&keyboards, "halcyon_elora_v2", |n| {
                keyboard::load(&keyboard_path(keyboards_dir, n))
            })
            .ok_or_else(|| {
                EngineError::Load(format!(
                    "no loadable keyboards in {}",
                    keyboards_dir.display()
                ))
            })?;

        let (initial_layout, layout) = pick_first_loadable(&layouts, "gallium-v2", |n| {
            load_layout_resolved(layouts_dir, n, &initial_keyboard)
        })
        .ok_or_else(|| {
            EngineError::Load(format!(
                "no loadable layouts in {}",
                layouts_dir.display()
            ))
        })?;

        let translator = translate::build(&layout, from_layout.as_deref());
        let events = open_events_store();

        // Heat for drill's starting-level inference. Scoped to the
        // current layout's canonical hash so switching layouts
        // doesn't leak heat across them.
        let heat = layout_heat(events.as_ref(), &layout, &initial_layout);

        let current_category = "drill".to_string();
        let current_instance = 0;
        let exercise = exercise_catalog::build(
            &current_category,
            current_instance,
            keyboard.as_ref(),
            &layout,
            &heat,
        );

        Ok(Engine {
            keyboards_dir: keyboards_dir.to_path_buf(),
            layouts_dir: layouts_dir.to_path_buf(),
            keyboards,
            layouts,
            current_keyboard: initial_keyboard,
            current_layout: initial_layout,
            current_category,
            current_instance,
            instance_memory: std::collections::HashMap::new(),
            keyboard,
            layout,
            exercise,
            translator,
            from_layout,
            events,
            events_session_pending: true,
            broken_keyboard: None,
            broken_layout: None,
            slot: KeyboardSlot::Keyboard,
            slot_visible: true,
            help_page_visible: false,
            stats_page_visible: false,
            layout_page_visible: false,
            active_stats_view: StatsView::Overview,
            stats_filter: StatsFilter::default(),
            active_overlay: Box::new(NoneOverlay),
            flash_enabled: false,
            last_flash: None,
        })
    }

    /* --- read accessors --- */

    pub fn current_keyboard(&self) -> &str {
        &self.current_keyboard
    }

    pub fn current_layout(&self) -> &str {
        &self.current_layout
    }

    /// Serialized form of the active exercise for prefs
    /// persistence. See `exercise::catalog::format_pref`.
    pub fn current_exercise(&self) -> String {
        exercise_catalog::format_pref(&self.current_category, self.current_instance)
    }

    /// Number of instances in the current category (0 for drill).
    pub fn current_instance_count(&self) -> usize {
        exercise_catalog::instance_count(&self.current_category)
    }

    /// Human label for the current instance, e.g. `"50"`,
    /// `"Endless"`, `"The Commit"`. `None` when the category has
    /// no instance axis.
    pub fn current_instance_label(&self) -> Option<String> {
        exercise_catalog::instance_label(&self.current_category, self.current_instance)
    }

    /* --- projection methods --- */

    /// Placements for terminal rendering (pos_a=c, pos_b=r).
    pub fn placements_for_terminal(&self) -> Vec<Placement> {
        project_for_terminal(self.keyboard.as_ref(), &self.layout)
    }

    /// Build the full DisplayState for a render.
    ///
    /// Uses field-by-field assignment on a `DisplayState::default()`
    /// even though clippy's `field_reassign_with_default` would
    /// prefer a struct-literal build: the trailing
    /// `exercise.fill_display(&mut display)` call requires a
    /// mutable binding to exist, and assembling the fields piecewise
    /// top-to-bottom reads more naturally given the conditionals
    /// around `exercise_instance` / slot / stats_view.
    #[allow(clippy::field_reassign_with_default)]
    pub fn display_state(&self) -> DisplayState {
        let mut display = DisplayState::default();
        display.keyboard_short = self.keyboard.short().to_string();
        display.layout_short = self.layout.short.clone();
        display.exercise_short = self.exercise.short().to_string();
        let instance_count = self.current_instance_count();
        display.exercise_instance = if instance_count == 0 {
            (0, 0)
        } else {
            (self.current_instance + 1, instance_count)
        };
        display.exercise_instance_label = self.current_instance_label();
        display.broken_keyboard = self.broken_keyboard.as_ref().map(Into::into);
        display.broken_layout = self.broken_layout.as_ref().map(Into::into);
        display.slot_visible = self.slot_visible;
        display.slot = match self.slot {
            KeyboardSlot::Keyboard => "keyboard",
            KeyboardSlot::InlineStats => "inline_stats",
        };
        display.help_page_visible = self.help_page_visible;
        display.stats_page_visible = self.stats_page_visible;
        display.layout_page_visible = self.layout_page_visible;
        display.stats_view = match self.active_stats_view {
            StatsView::Overview => "overview",
            StatsView::Progression => "progression",
            StatsView::LayoutView => "layout_view",
        };
        display.overlay_name = self.active_overlay.name();

        let live = self.session_live_stats();
        // DisplayState's `session_accuracy` is historically a
        // percentage (0..=100), not a fraction — the footer format
        // string assumes that. `SessionLive::accuracy()` gives the
        // clean fraction, so convert at the boundary.
        display.session_accuracy = live.accuracy() * 100.0;
        display.session_total_correct = live.total_correct;
        display.session_total_wrong = live.total_wrong;
        let speed = self.session_wpm_stats();
        display.session_wpm = speed.net_wpm();
        display.session_apm = speed.apm();

        self.exercise.fill_display(&mut display);
        display
    }

    /* --- input --- */

    /// Translate + evaluate + record + advance exercise.
    pub fn process_input(&mut self, ch: char) -> KeystrokeResult {
        let translated = self.translator.translate(ch);
        // Record the flash regardless of whether an exercise is
        // active — we still want the visual feedback for arbitrary
        // keystrokes. The renderer gates on `flash_enabled`.
        self.last_flash = Some(FlashKeystroke {
            char: translated.to_ascii_lowercase(),
            at: std::time::Instant::now(),
        });
        let Some(expected) = self.exercise.expected() else {
            return KeystrokeResult {
                hit: false,
                exercise_done: self.exercise.is_done(),
            };
        };
        let hit = translated == expected;

        // Lazy session-start keeps the sessions table free of
        // zero-event rows for launches where the user toggled
        // layouts without typing.
        let now_ms = now_unix_ms();
        if self.events_session_pending {
            self.begin_events_session(now_ms);
            self.events_session_pending = false;
        }
        let recorded_event = if let Some(events) = self.events.as_mut() {
            match events.record(expected, translated, now_ms) {
                Ok(()) => Some(keywiz_stats::Event {
                    session_id: events.current_session().unwrap_or(keywiz_stats::SessionId(0)),
                    ts_ms: now_ms,
                    expected,
                    typed: translated,
                    correct: hit,
                    delta_ms: None,
                }),
                Err(e) => {
                    eprintln!("keywiz-stats: record failed: {e:#}");
                    None
                }
            }
        } else {
            None
        };

        // Exercise needs a current heat snapshot for drill's
        // autoscaler. Re-querying every keystroke is cheap: ~hundreds
        // of events, one HashMap build, microseconds.
        let heat = layout_heat(self.events.as_ref(), &self.layout, &self.current_layout);
        self.exercise.advance(&heat, hit);

        // Let the active overlay refresh per-event caches (heat
        // map, accuracy trail, etc.). Split-field borrow lets us
        // hand a `&dyn EventStore` from `self.events` to the
        // overlay while holding `self.active_overlay` mutably.
        if let (Some(event), Some(events)) = (recorded_event, self.events.as_ref()) {
            let layout_hash = layout_snapshot(&self.layout, &self.current_layout, 0).hash;
            let ctx = crate::renderer::overlay::OverlayContext {
                store: events.store(),
                layout_hash: &layout_hash,
            };
            self.active_overlay.on_event(&event, &ctx);
        }
        KeystrokeResult {
            hit,
            exercise_done: self.exercise.is_done(),
        }
    }

    /// Open an event-stream session for the current (keyboard,
    /// layout, exercise) context. Safe to call multiple times — if
    /// a session is already active, it's closed first and a new
    /// one opens with the fresh context. No-op when the events
    /// store failed to open at startup.
    fn begin_events_session(&mut self, now_ms: i64) {
        if self.events.is_none() {
            return;
        }
        // Compute everything that borrows self immutably *before*
        // taking the mutable borrow on self.events.
        let layout_snap = layout_snapshot(&self.layout, &self.current_layout, now_ms);
        let keyboard_snap =
            keyboard_snapshot(self.keyboard.as_ref(), &self.current_keyboard, now_ms);
        let instance_label = self.current_instance_label();
        let category = self.current_category.clone();
        let events = self.events.as_mut().expect("checked above");
        if let Err(e) = events.begin_session(
            &layout_snap,
            &keyboard_snap,
            &category,
            instance_label.as_deref(),
            now_ms,
        ) {
            eprintln!("keywiz-stats: begin_session failed: {e:#}");
        }
    }

    /// Close the current event-stream session (if any) and mark a
    /// new one as pending. Called from every context-switch path
    /// (set_keyboard, set_layout, set_category_instance) and from
    /// the graceful-shutdown hook.
    pub fn end_events_session(&mut self) {
        let now_ms = now_unix_ms();
        if let Some(events) = self.events.as_mut()
            && events.current_session().is_some()
            && let Err(e) = events.end_session(now_ms)
        {
            eprintln!("keywiz-stats: end_session failed: {e:#}");
        }
        self.events_session_pending = true;
    }

    /* --- display toggles --- */

    /// Tab — hide/show whatever's currently in the keyboard slot.
    /// Real toggle, not a cycle: Tab from visible hides it; Tab
    /// from hidden restores whatever slot was last active (F4
    /// cycles are preserved across hide/show).
    pub fn toggle_slot_visible(&mut self) {
        self.slot_visible = !self.slot_visible;
    }

    /// F4 — cycle what's shown in the keyboard slot (keyboard ↔
    /// inline stats ↔ ...). Doesn't change visibility; if Tab
    /// hid the slot, the next F4 still runs silently — you see
    /// the new slot content next time Tab reveals it.
    pub fn cycle_slot(&mut self) {
        self.slot = self.slot.cycle_next();
    }

    /// F1 — enter/leave the help page. Typing is paused.
    pub fn toggle_help_page(&mut self) {
        self.help_page_visible = !self.help_page_visible;
    }

    /// Whether the help page is currently showing.
    pub fn help_page_visible(&self) -> bool {
        self.help_page_visible
    }

    /// F4 — enter/leave the full stats page. When active, typing
    /// is paused and the stats page replaces the usual surfaces.
    pub fn toggle_stats_page(&mut self) {
        self.stats_page_visible = !self.stats_page_visible;
    }

    /// Advance to the next stats view inside the F3 page. No-op
    /// when the stats page isn't visible (cycling is modal).
    pub fn next_stats_view(&mut self) {
        if self.stats_page_visible {
            self.active_stats_view = self.active_stats_view.cycle_next();
        }
    }

    /// Back to the previous stats view.
    pub fn prev_stats_view(&mut self) {
        if self.stats_page_visible {
            self.active_stats_view = self.active_stats_view.cycle_prev();
        }
    }

    /// Whether the full stats page is currently showing. Main loop
    /// consults this to route keybinds modally.
    pub fn stats_page_visible(&self) -> bool {
        self.stats_page_visible
    }

    /// F5 — enter/leave the layout-iterations modal.
    pub fn toggle_layout_page(&mut self) {
        self.layout_page_visible = !self.layout_page_visible;
    }

    /// Whether the layout-iterations modal is currently showing.
    pub fn layout_page_visible(&self) -> bool {
        self.layout_page_visible
    }

    /// Borrow the active stats filter. Stats pages read this to
    /// scope their queries + render the breadcrumb header.
    pub fn stats_filter(&self) -> &StatsFilter {
        &self.stats_filter
    }

    /// Ctrl+← inside the stats modal — cycle to the previous
    /// (layout, keyboard) combination that actually exists in the
    /// event store. Wraps through an "all combos" sentinel at the
    /// top of the list.
    pub fn stats_prev_combo(&mut self) {
        let combos = self.known_combos();
        self.stats_filter.combo = cycle_combo_prev(&combos, self.stats_filter.combo.as_ref());
    }

    /// Ctrl+→ inside the stats modal — cycle to the next combo.
    pub fn stats_next_combo(&mut self) {
        let combos = self.known_combos();
        self.stats_filter.combo = cycle_combo_next(&combos, self.stats_filter.combo.as_ref());
    }

    /// Enumerate the (layout_name, keyboard_name) combos present
    /// in the sessions table, sorted for stable cycle order.
    /// Returns an empty list when no events store is open — the
    /// footer shows "no combos" and cycling is a no-op.
    fn known_combos(&self) -> Vec<Combo> {
        let Some(events) = self.events.as_ref() else {
            return Vec::new();
        };
        let filter = keywiz_stats::SessionFilter::default();
        let sessions = match events.store().sessions(&filter) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let mut seen: std::collections::BTreeSet<(String, String)> = Default::default();
        for session in sessions {
            seen.insert((session.layout_name, session.keyboard_name));
        }
        seen.into_iter()
            .map(|(layout, keyboard)| Combo { layout, keyboard })
            .collect()
    }

    /// Resolve the active combo filter to the set of session ids
    /// whose (layout_name, keyboard_name) matches. Returns `None`
    /// when the filter is unconstrained (all combos); returns
    /// `Some(vec)` — possibly empty — when a specific combo is
    /// selected. An empty vec means "this combo exists in the
    /// cycle list but has no sessions in the active time window"
    /// (e.g., today's buckets with yesterday's layout).
    fn resolve_combo_sessions(&self) -> Option<Vec<keywiz_stats::SessionId>> {
        let combo = self.stats_filter.combo.as_ref()?;
        let events = self.events.as_ref()?;
        let filter = keywiz_stats::SessionFilter {
            layout_name: Some(combo.layout.clone()),
            ..Default::default()
        };
        let sessions = events.store().sessions(&filter).ok()?;
        Some(
            sessions
                .into_iter()
                .filter(|s| s.keyboard_name == combo.keyboard)
                .map(|s| s.session_id)
                .collect(),
        )
    }

    /// Alt+↑ inside the stats modal — previous granularity (current
    /// session → day → week → month → year → all).
    pub fn stats_prev_granularity(&mut self) {
        self.stats_filter.prev_granularity();
    }

    /// Alt+↓ inside the stats modal — next granularity.
    pub fn stats_next_granularity(&mut self) {
        self.stats_filter.next_granularity();
    }

    /// Alt+← inside the stats modal. P1/P3: newer bucket (closer
    /// to now). P2: narrower range (fewer buckets plotted).
    pub fn stats_newer_offset(&mut self) {
        if matches!(self.active_stats_view, StatsView::Progression) {
            self.stats_filter.narrower_range();
        } else {
            self.stats_filter.newer_offset();
        }
    }

    /// Alt+→ inside the stats modal. P1/P3: older bucket (further
    /// back). P2: wider range (more buckets plotted).
    pub fn stats_older_offset(&mut self) {
        if matches!(self.active_stats_view, StatsView::Progression) {
            self.stats_filter.wider_range();
        } else {
            self.stats_filter.older_offset();
        }
    }

    /// Aggregate per-iteration stats for the active filter's combo.
    ///
    /// Every time the user swaps keys on a layout, the canonical
    /// hash changes — so a single name like "drifter" can cover
    /// multiple iterations. This method groups all of that
    /// combo's sessions by `layout_hash` and rolls up headline
    /// numbers per iteration, ordered oldest → newest by first
    /// session timestamp.
    ///
    /// Returns `None` when the filter has no specific combo
    /// selected (iterations are only meaningful within one
    /// layout's name). Returns an empty vec when the combo exists
    /// but the store found no sessions.
    pub fn iteration_stats(&self) -> Option<Vec<IterationStats>> {
        let combo = self.stats_filter.combo.as_ref()?;
        let events = self.events.as_ref()?;

        // Pull all sessions for this layout name. The session
        // filter accepts `layout_name` directly; keyboard is a
        // second pass since there's no keyboard-name filter on
        // SessionFilter.
        let session_filter = keywiz_stats::SessionFilter {
            layout_name: Some(combo.layout.clone()),
            ..Default::default()
        };
        let sessions = events.store().sessions(&session_filter).ok()?;

        // Group by layout_hash, accumulating per-iteration stats
        // from matching sessions (filter keyboard here).
        let mut by_hash: std::collections::HashMap<
            keywiz_stats::LayoutHash,
            IterationStats,
        > = std::collections::HashMap::new();
        let mut session_ids_by_hash: std::collections::HashMap<
            keywiz_stats::LayoutHash,
            Vec<keywiz_stats::SessionId>,
        > = std::collections::HashMap::new();

        for session in sessions {
            if session.keyboard_name != combo.keyboard {
                continue;
            }
            let entry = by_hash.entry(session.layout_hash.clone()).or_insert_with(|| {
                IterationStats {
                    hash: session.layout_hash.clone(),
                    first_seen_ms: session.started_at_ms,
                    last_seen_ms: session.ended_at_ms.unwrap_or(session.started_at_ms),
                    session_count: 0,
                    total_events: 0,
                    total_correct: 0,
                    active_ms: 0,
                }
            });
            entry.session_count += 1;
            entry.first_seen_ms = entry.first_seen_ms.min(session.started_at_ms);
            if let Some(ended) = session.ended_at_ms {
                entry.last_seen_ms = entry.last_seen_ms.max(ended);
            } else {
                entry.last_seen_ms = entry.last_seen_ms.max(session.started_at_ms);
            }
            session_ids_by_hash
                .entry(session.layout_hash.clone())
                .or_default()
                .push(session.session_id);
        }

        // Second pass: walk events for each iteration to sum
        // active_ms (which SessionSummary doesn't store). The
        // session-level `total_events` / `total_correct` already
        // live on `SessionSummary`, but the events pass gives us
        // those same counts plus timing in one trip.
        for (hash, ids) in session_ids_by_hash {
            let filter = keywiz_stats::EventFilter {
                session_ids: Some(ids),
                ..Default::default()
            };
            let Ok(iter) = events.store().events(&filter) else {
                continue;
            };
            let entry = by_hash.get_mut(&hash).expect("populated above");
            for ev in iter.flatten() {
                entry.total_events += 1;
                if ev.correct {
                    entry.total_correct += 1;
                }
                if let Some(ms) = ev.delta_ms {
                    entry.active_ms += ms as u64;
                }
            }
        }

        let mut out: Vec<IterationStats> = by_hash.into_values().collect();
        out.sort_by_key(|s| s.first_seen_ms);
        Some(out)
    }

    /// Per-finger load + miss count for the active filter scope.
    /// Joins events' `expected` chars through the current layout +
    /// keyboard to find which finger each keystroke used.
    ///
    /// The current live layout/keyboard drive the mapping even when
    /// the filter scopes to another combo's hash — this matches the
    /// user's question ("how does *my hands* use this data"). A
    /// future improvement is to look up the session's own keyboard
    /// snapshot for authoritative mapping per-session; that's a
    /// bigger lift once per-snapshot finger decoding exists.
    pub fn finger_load(&self) -> std::collections::HashMap<crate::keyboard::common::Finger, FingerStats> {
        use std::collections::HashMap;
        let mut out: HashMap<crate::keyboard::common::Finger, FingerStats> = HashMap::new();
        let Some(store) = self.events_store() else {
            return out;
        };
        let Some(filter) = self.resolve_event_filter() else {
            return out;
        };
        let char_to_finger = char_finger_map(self.keyboard.as_ref(), &self.layout);
        for ev in store.events(&filter).into_iter().flatten().flatten() {
            let key = ev.expected.to_ascii_lowercase();
            let Some(finger) = char_to_finger.get(&key).copied() else {
                continue;
            };
            let stats = out.entry(finger).or_default();
            stats.count += 1;
            if !ev.correct {
                stats.miss_count += 1;
            }
        }
        out
    }

    /// Run the progression aggregator for the active filter. Returns
    /// one [`BucketStats`](keywiz_stats::views::progression::BucketStats)
    /// per bucket at the current (granularity, range), ordered
    /// oldest → newest. Returns an empty vec when no store is open
    /// or the granularity doesn't support multi-bucket views.
    pub fn progression_buckets(
        &self,
    ) -> Vec<keywiz_stats::views::progression::BucketStats> {
        let Some(events) = self.events.as_ref() else {
            return Vec::new();
        };
        let now_ms = now_unix_ms();
        let ranges = granularity_range(
            self.stats_filter.granularity,
            self.stats_filter.range,
            now_ms,
        );
        if ranges.is_empty() {
            return Vec::new();
        }
        // Base filter carries combo scoping only — the bucket
        // aggregator overrides from_ms/until_ms per bucket.
        let mut base = keywiz_stats::EventFilter::default();
        if let Some(ids) = self.resolve_combo_sessions() {
            base.session_ids = Some(ids);
        }
        keywiz_stats::views::progression::bucket_stats(events.store(), &base, &ranges)
            .unwrap_or_default()
    }

    /// Resolve the active `StatsFilter` to a concrete
    /// `EventFilter`. Returns `None` when no events store is open.
    ///
    /// Granularity translates as follows:
    ///
    /// - `CurrentSession` — scoped to the active session id, or
    ///   returns `None` when no session is running yet.
    /// - `Day` / `Week` / `Month` / `Year` — scoped to a
    ///   `[from_ms, until_ms)` window around "now + offset" in the
    ///   selected unit.
    /// - `All` — no time bound; returns every event.
    ///
    /// Layout/keyboard name filtering is a session-level property
    /// (events don't denormalize it). Resolving a name to the set
    /// of matching layout hashes lands when P2 needs cross-hash
    /// queries.
    pub fn resolve_event_filter(&self) -> Option<keywiz_stats::EventFilter> {
        let events = self.events.as_ref()?;
        let mut filter = keywiz_stats::EventFilter::default();
        let now_ms = now_unix_ms();
        match self.stats_filter.granularity {
            Granularity::CurrentSession => {
                filter.session_id = Some(events.current_session()?);
            }
            Granularity::All => {}
            Granularity::Day | Granularity::Week | Granularity::Month | Granularity::Year => {
                let (from, until) = granularity_window(
                    self.stats_filter.granularity,
                    self.stats_filter.offset,
                    now_ms,
                );
                filter.from_ms = Some(from);
                filter.until_ms = Some(until);
            }
        }
        // Combo narrowing composes with the time window.
        if let Some(ids) = self.resolve_combo_sessions() {
            filter.session_ids = Some(ids);
        }
        Some(filter)
    }

    /// Borrow the underlying events store, if one is open. Stats
    /// pages call this to run view queries against the event
    /// stream. `None` when the SQLite store couldn't open at
    /// startup — pages must render empty states in that case.
    pub fn events_store(&self) -> Option<&dyn keywiz_stats::EventStore> {
        self.events.as_ref().map(|s| s.store())
    }

    /// Active stats view (which of the three pages is showing).
    /// Staged — the dispatcher reads `DisplayState::stats_view`
    /// today; this accessor is the typed alternative once pages
    /// need to branch without stringly-typed comparisons.
    #[allow(dead_code)]
    pub fn active_stats_view(&self) -> StatsView {
        self.active_stats_view
    }


    /// Cycle F2 through overlay modes: none → finger → heat → none.
    /// Entering the heat overlay triggers a fresh heat-map build
    /// from the event stream, scoped to the current layout hash so
    /// historical data from other iterations doesn't pollute.
    pub fn cycle_overlay(&mut self) {
        let next = match self.active_overlay.name() {
            "none" => "finger",
            "finger" => "heat",
            _ => "none",
        };
        self.active_overlay = self.build_overlay(next);
    }

    /// Set the active overlay from a stored preference string.
    /// Unknown names fall through to the none overlay silently —
    /// prefs files written by older keywiz versions shouldn't
    /// cause failures on load.
    pub fn set_overlay_by_name(&mut self, name: &str) {
        self.active_overlay = self.build_overlay(name);
    }

    /// Borrow the active overlay. Main loop hands this to the
    /// renderer each frame.
    pub fn active_overlay(&self) -> &dyn KeyOverlay {
        self.active_overlay.as_ref()
    }

    /// Shift+Tab — toggle the flash-on-keypress layer.
    pub fn toggle_flash(&mut self) {
        self.flash_enabled = !self.flash_enabled;
    }

    /// Whether the flash layer is currently active. Renderer reads
    /// this to decide whether to composite the last-keystroke flash.
    pub fn flash_enabled(&self) -> bool {
        self.flash_enabled
    }

    /// The most recent keystroke, if any. Renderer mixes its age
    /// against a fade duration to draw a bright border on the
    /// matching key.
    pub fn last_flash(&self) -> Option<FlashKeystroke> {
        self.last_flash
    }

    fn build_overlay(&self, name: &str) -> Box<dyn KeyOverlay> {
        match name {
            "finger" => Box::new(FingerOverlay::new(FingerStyle::default())),
            "heat" => {
                let map = self
                    .events
                    .as_ref()
                    .and_then(|events| {
                        let layout_hash = crate::stats_adapter::layout_snapshot(
                            &self.layout,
                            &self.current_layout,
                            0,
                        )
                        .hash;
                        let filter = keywiz_stats::EventFilter {
                            layout_hash: Some(layout_hash),
                            ..Default::default()
                        };
                        keywiz_stats::views::heat::heat_map(events.store(), &filter).ok()
                    })
                    .unwrap_or_default();
                Box::new(HeatOverlay::new(map, HeatStyle::default()))
            }
            _ => Box::new(NoneOverlay),
        }
    }

    /* --- setters (keyboard / layout / exercise) --- */

    pub fn set_keyboard(&mut self, name: &str) -> Result<(), EngineError> {
        if !self.keyboards.iter().any(|n| n == name) {
            return Err(EngineError::UnknownKeyboard(name.to_string()));
        }
        // Close the outgoing session before the (keyboard, layout)
        // pair changes — session identity is pinned to that pair.
        self.end_events_session();
        let keyboard = match keyboard::load(&keyboard_path(&self.keyboards_dir, name)) {
            Ok(k) => k,
            Err(reason) => {
                self.broken_keyboard = Some(BrokenSelection {
                    name: name.to_string(),
                    reason: reason.clone(),
                });
                self.current_keyboard = name.to_string();
                return Err(EngineError::Broken {
                    name: name.to_string(),
                    message: reason,
                });
            }
        };
        let layout = match load_layout_resolved(&self.layouts_dir, &self.current_layout, name) {
            Ok(l) => l,
            Err(reason) => {
                self.broken_keyboard = Some(BrokenSelection {
                    name: name.to_string(),
                    reason: reason.clone(),
                });
                self.current_keyboard = name.to_string();
                return Err(EngineError::Broken {
                    name: name.to_string(),
                    message: reason,
                });
            }
        };
        self.broken_keyboard = None;
        self.current_keyboard = name.to_string();
        self.keyboard = keyboard;
        self.layout = layout;
        self.rebuild_exercise();
        self.rebuild_translator();
        Ok(())
    }

    pub fn set_layout(&mut self, name: &str) -> Result<LayoutChange, EngineError> {
        if !self.layouts.iter().any(|n| n == name) {
            return Err(EngineError::UnknownLayout(name.to_string()));
        }
        self.end_events_session();
        let keyboard = match keyboard::load(&keyboard_path(
            &self.keyboards_dir,
            &self.current_keyboard,
        )) {
            Ok(k) => k,
            Err(message) => return Err(EngineError::Load(message)),
        };
        let layout = match load_layout_resolved(&self.layouts_dir, name, &self.current_keyboard) {
            Ok(l) => l,
            Err(reason) => {
                self.broken_layout = Some(BrokenSelection {
                    name: name.to_string(),
                    reason: reason.clone(),
                });
                let change = LayoutChange {
                    from: std::mem::replace(&mut self.current_layout, name.to_string()),
                };
                return Err(EngineError::Broken {
                    name: name.to_string(),
                    message: format!("{reason} (from: {})", change.from),
                });
            }
        };
        self.broken_layout = None;
        // keywiz-stats already persists per layout via content hash —
        // no separate save needed on layout switch.
        let change = LayoutChange {
            from: std::mem::replace(&mut self.current_layout, name.to_string()),
        };
        self.keyboard = keyboard;
        self.layout = layout;
        self.rebuild_exercise();
        self.rebuild_translator();
        Ok(change)
    }

    /// Set the active exercise from a serialized prefs string
    /// (`"text:3"`, `"words:50"`, `"drill"`, or any legacy name).
    /// Safe against unknown formats — falls back to drill.
    pub fn set_exercise_from_pref(&mut self, pref: &str) {
        let (cat, inst) = exercise_catalog::parse_pref(pref);
        self.set_category_instance(cat, inst);
    }

    fn set_category_instance(&mut self, category: String, instance: usize) {
        // Exercise switch = new session. Close outgoing first.
        self.end_events_session();
        // Remember where we were in the outgoing category before
        // moving on.
        self.instance_memory
            .insert(self.current_category.clone(), self.current_instance);
        self.current_category = category;
        // Clamp instance to the category's range.
        let bound = exercise_catalog::instance_count(&self.current_category);
        self.current_instance = if bound == 0 { 0 } else { instance.min(bound - 1) };
        self.rebuild_exercise();
    }

    fn rebuild_exercise(&mut self) {
        let heat = layout_heat(self.events.as_ref(), &self.layout, &self.current_layout);
        self.exercise = exercise_catalog::build(
            &self.current_category,
            self.current_instance,
            self.keyboard.as_ref(),
            &self.layout,
            &heat,
        );
    }

    fn rebuild_translator(&mut self) {
        self.translator = translate::build(&self.layout, self.from_layout.as_deref());
    }

    /* --- cycling --- */

    pub fn next_keyboard(&mut self) -> Result<(), EngineError> {
        let Some(name) = next_in(&self.keyboards, &self.current_keyboard) else {
            return Ok(());
        };
        self.set_keyboard(&name)
    }

    pub fn prev_keyboard(&mut self) -> Result<(), EngineError> {
        let Some(name) = prev_in(&self.keyboards, &self.current_keyboard) else {
            return Ok(());
        };
        self.set_keyboard(&name)
    }

    pub fn next_layout(&mut self) -> Result<LayoutChange, EngineError> {
        let name = next_in(&self.layouts, &self.current_layout)
            .ok_or_else(|| EngineError::UnknownLayout(self.current_layout.clone()))?;
        self.set_layout(&name)
    }

    pub fn prev_layout(&mut self) -> Result<LayoutChange, EngineError> {
        let name = prev_in(&self.layouts, &self.current_layout)
            .ok_or_else(|| EngineError::UnknownLayout(self.current_layout.clone()))?;
        self.set_layout(&name)
    }

    /// Cycle to the next exercise category (Alt+↓). Restores the
    /// new category's remembered instance if one exists.
    pub fn next_exercise_category(&mut self) {
        let next = exercise_catalog::next_category(&self.current_category).to_string();
        let inst = self.remembered_instance(&next);
        self.set_category_instance(next, inst);
    }

    /// Cycle to the previous exercise category (Alt+↑).
    pub fn prev_exercise_category(&mut self) {
        let prev = exercise_catalog::prev_category(&self.current_category).to_string();
        let inst = self.remembered_instance(&prev);
        self.set_category_instance(prev, inst);
    }

    /// Cycle to the next instance within the current category
    /// (Alt+→). No-op when the category has no instances (drill).
    pub fn next_exercise_instance(&mut self) {
        if let Some(next) =
            exercise_catalog::next_instance(&self.current_category, self.current_instance)
        {
            self.current_instance = next;
            self.rebuild_exercise();
        }
    }

    /// Cycle to the previous instance within the current category
    /// (Alt+←).
    pub fn prev_exercise_instance(&mut self) {
        if let Some(prev) =
            exercise_catalog::prev_instance(&self.current_category, self.current_instance)
        {
            self.current_instance = prev;
            self.rebuild_exercise();
        }
    }

    fn remembered_instance(&self, category: &str) -> usize {
        self.instance_memory.get(category).copied().unwrap_or(0)
    }

    /// Live tally for the active session — what the footer paints
    /// as "Correct / Wrong / Accuracy." Returns zeros when no
    /// session is running (first keystroke hasn't landed yet).
    fn session_live_stats(&self) -> keywiz_stats::views::session_live::SessionLive {
        let Some(events) = self.events.as_ref() else {
            return keywiz_stats::views::session_live::SessionLive::default();
        };
        let Some(sid) = events.current_session() else {
            return keywiz_stats::views::session_live::SessionLive::default();
        };
        keywiz_stats::views::session_live::live_for(events.store(), sid).unwrap_or_default()
    }

    /// Live WPM/APM for the active session. Returns zeros when no
    /// session is running or when fewer than two events have been
    /// recorded (need at least one `delta_ms` to have meaningful
    /// timing).
    fn session_wpm_stats(&self) -> keywiz_stats::views::wpm::SessionWpm {
        let Some(events) = self.events.as_ref() else {
            return keywiz_stats::views::wpm::SessionWpm::default();
        };
        let Some(sid) = events.current_session() else {
            return keywiz_stats::views::wpm::SessionWpm::default();
        };
        keywiz_stats::views::wpm::live_for(events.store(), sid).unwrap_or_default()
    }
}

/// Build the per-char heat map for the current layout. Scoped to
/// the layout's content hash so heat from previous iterations of
/// the same-named layout doesn't leak in.
fn layout_heat(
    events: Option<&keywiz_stats::Stats>,
    layout: &Layout,
    display_name: &str,
) -> HashMap<char, u32> {
    let Some(events) = events else {
        return HashMap::new();
    };
    let snapshot = layout_snapshot(layout, display_name, 0);
    let filter = keywiz_stats::EventFilter {
        layout_hash: Some(snapshot.hash),
        ..Default::default()
    };
    keywiz_stats::views::heat::heat_map_raw(events.store(), &filter).unwrap_or_default()
}

/// Compute a `[from_ms, until_ms)` calendar bucket for a
/// granularity + offset, anchored at `now_ms`. Offset `0` picks the
/// current bucket (today / this ISO week / this month / this year);
/// `-1` picks the previous bucket; positive offsets reach into the
/// (empty) future.
///
/// Uses the system's local timezone via [`chrono::Local`] because
/// "this week" is a human question about when the user was typing
/// — not a UTC query. The first day of a week is Monday per ISO
/// 8601.
///
/// Returns `(i64::MIN, i64::MAX)` for `CurrentSession` / `All` —
/// those paths are filtered out earlier; this arm exists so the
/// function is total.
fn granularity_window(g: Granularity, offset: i64, now_ms: i64) -> (i64, i64) {
    use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Timelike};

    let Some(now) = chrono::DateTime::from_timestamp_millis(now_ms) else {
        return (i64::MIN, i64::MAX);
    };
    let now = now.with_timezone(&Local);

    let (from_date, until_date) = match g {
        Granularity::Day => {
            let start = now.date_naive() + Duration::days(offset);
            (start, start + Duration::days(1))
        }
        Granularity::Week => {
            // ISO week starts Monday. Roll `now` back to the most
            // recent Monday, then offset by `offset` weeks.
            let weekday = now.weekday().num_days_from_monday() as i64;
            let monday = now.date_naive() - Duration::days(weekday);
            let start = monday + Duration::weeks(offset);
            (start, start + Duration::weeks(1))
        }
        Granularity::Month => {
            let start = shift_month(now.year(), now.month() as i32, offset as i32);
            let end = shift_month(start.year(), start.month() as i32, 1);
            (start, end)
        }
        Granularity::Year => {
            let year = now.year() + offset as i32;
            let start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap_or(now.date_naive());
            let end =
                NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap_or(now.date_naive());
            (start, end)
        }
        Granularity::CurrentSession | Granularity::All => {
            return (i64::MIN, i64::MAX);
        }
    };

    let to_ms = |d: NaiveDate| -> i64 {
        Local
            .from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap_or_default())
            .single()
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(0)
    };

    // Guard against suspicious clocks.
    let _ = now.hour();
    (to_ms(from_date), to_ms(until_date))
}

/// Return the first-of-month `NaiveDate` for `year`/`month` shifted
/// by `delta_months`. Clamps December → January rollovers correctly.
fn shift_month(year: i32, month: i32, delta_months: i32) -> chrono::NaiveDate {
    let mut y = year;
    let mut m = month + delta_months;
    while m <= 0 {
        m += 12;
        y -= 1;
    }
    while m > 12 {
        m -= 12;
        y += 1;
    }
    chrono::NaiveDate::from_ymd_opt(y, m as u32, 1).unwrap_or_else(|| {
        chrono::NaiveDate::from_ymd_opt(year, month as u32, 1)
            .unwrap_or_default()
    })
}

/// Compute N consecutive `[from, until)` bucket ranges ending at
/// `offset=0` (most-recent bucket). Used by the progression page
/// to ask "last N days" / "last N weeks" / etc.
///
/// Returns buckets in chronological order (oldest → newest).
/// Returns an empty vec for `CurrentSession` or `All` — those
/// granularities have no meaningful multi-bucket view.
pub fn granularity_range(g: Granularity, count: usize, now_ms: i64) -> Vec<(i64, i64)> {
    if count == 0
        || matches!(g, Granularity::CurrentSession | Granularity::All)
    {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(count);
    for offset in (0..count as i64).rev() {
        out.push(granularity_window(g, -offset, now_ms));
    }
    out
}

/// Aggregate stats for one iteration of a layout — one `layout_hash`
/// rolled up across every session that ran it under the active
/// combo's keyboard. Returned by [`Engine::iteration_stats`] and
/// consumed by the F5 layout page.
#[derive(Debug, Clone)]
pub struct IterationStats {
    pub hash: keywiz_stats::LayoutHash,
    pub first_seen_ms: i64,
    pub last_seen_ms: i64,
    pub session_count: u64,
    pub total_events: u64,
    pub total_correct: u64,
    /// Sum of `delta_ms` across timed events — used to compute
    /// net WPM + APM for the iteration.
    pub active_ms: u64,
}

impl IterationStats {
    /// Net WPM: correct chars / 5 / minutes of active typing.
    /// Zero when no timed events contributed.
    pub fn net_wpm(&self) -> f64 {
        if self.active_ms == 0 {
            return 0.0;
        }
        let minutes = self.active_ms as f64 / 60_000.0;
        (self.total_correct as f64 / 5.0) / minutes
    }

    /// Accuracy as a percentage 0..=100. 100 on an empty iteration
    /// (mirrors [`BucketStats::accuracy_pct`]).
    ///
    /// [`BucketStats::accuracy_pct`]: keywiz_stats::views::progression::BucketStats::accuracy_pct
    pub fn accuracy_pct(&self) -> f64 {
        if self.total_events == 0 {
            return 100.0;
        }
        (self.total_correct as f64 / self.total_events as f64) * 100.0
    }
}

/// Aggregate stats for a single finger — count of keystrokes
/// attributed to it (load) and how many of those were wrong.
#[derive(Debug, Clone, Copy, Default)]
pub struct FingerStats {
    /// Total keystrokes the user was asked to type on this finger.
    pub count: u64,
    /// Of those, keystrokes where the user typed the wrong char.
    pub miss_count: u64,
}

impl FingerStats {
    /// Miss rate 0.0..=1.0. Returns 0.0 on empty.
    pub fn miss_rate(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.miss_count as f64 / self.count as f64
    }
}

/// Build a `char → Finger` map from the live layout + keyboard.
/// Used by [`Engine::finger_load`] so the per-finger rollup is
/// independent of the physical key ids (layouts may remap keys; we
/// go via char).
///
/// Both the lowercase and uppercase variants of a char point at
/// the same finger, matching the heat/bigram convention where
/// case folds.
fn char_finger_map(
    keyboard: &dyn crate::keyboard::Keyboard,
    layout: &crate::mapping::Layout,
) -> std::collections::HashMap<char, crate::keyboard::common::Finger> {
    use std::collections::HashMap;
    let mut id_to_finger: HashMap<String, crate::keyboard::common::Finger> = HashMap::new();
    for key in keyboard.keys() {
        id_to_finger.insert(key.id.clone(), key.finger);
    }
    let mut out: HashMap<char, crate::keyboard::common::Finger> = HashMap::new();
    for (id, mapping) in &layout.mappings {
        let Some(finger) = id_to_finger.get(id).copied() else {
            continue;
        };
        if let crate::mapping::KeyMapping::Char { lower, upper } = mapping {
            out.insert(lower.to_ascii_lowercase(), finger);
            out.insert(upper.to_ascii_lowercase(), finger);
        }
    }
    out
}

/// Cycle forward through a combo list. `None` ("all combos") sits
/// at the top of the cycle — advancing from `None` picks the first
/// combo, advancing off the last wraps back to `None`. An unknown
/// current combo (one that's been filtered out of the DB since
/// last selection) resets to `None`.
fn cycle_combo_next(combos: &[Combo], current: Option<&Combo>) -> Option<Combo> {
    let Some(cur) = current else {
        return combos.first().cloned();
    };
    let idx = combos.iter().position(|c| c == cur)?;
    combos.get(idx + 1).cloned()
}

/// Cycle backward through a combo list.
fn cycle_combo_prev(combos: &[Combo], current: Option<&Combo>) -> Option<Combo> {
    let Some(cur) = current else {
        return combos.last().cloned();
    };
    let idx = combos.iter().position(|c| c == cur)?;
    if idx == 0 {
        None
    } else {
        combos.get(idx - 1).cloned()
    }
}

/// Millis since Unix epoch, clamped to a signed i64. System
/// clock going backwards is rare enough that we don't bother
/// guarding against it; the store uses monotonic ordering *within*
/// a session, and sessions are small.
fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Open the SQLite event store at the user's data directory and
/// reconcile any stale sessions (ones whose `ended_at_ms` is NULL
/// because the previous run crashed / was killed). Returns `None`
/// on any I/O or schema error — keywiz stays usable, just without
/// the new stats layer for this run.
fn open_events_store() -> Option<keywiz_stats::Stats> {
    let dir = dirs::data_dir()?;
    let path = dir.join(EVENTS_STORE_RELPATH);
    if let Some(parent) = path.parent()
        && !parent.exists()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!(
            "keywiz-stats: could not create data dir {}: {e}",
            parent.display()
        );
        return None;
    }
    match keywiz_stats::store::sqlite::SqliteStore::open(&path) {
        Ok(store) => Some(keywiz_stats::Stats::new(Box::new(store))),
        Err(e) => {
            eprintln!(
                "keywiz-stats: could not open event store at {}: {e:#}",
                path.display()
            );
            None
        }
    }
}
