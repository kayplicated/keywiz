//! The active scope for stats-page queries — which layout, which
//! keyboard, and which time slice.
//!
//! Every stats page reads one `StatsFilter` + translates it into an
//! [`EventFilter`] / [`SessionFilter`] before querying. The filter is
//! owned by the engine and cycled via the same Ctrl+±/Alt+± keys
//! that drive layout/keyboard/exercise cycling in the typing view —
//! same physical keys, context-appropriate meaning.
//!
//! Phase 1 ships `Granularity::CurrentSession` as the only supported
//! slice; wider granularities (Day / Week / Month / Year / All) land
//! with P2 Progression when cross-session aggregation is wired.
//!
//! [`EventFilter`]: keywiz_stats::EventFilter
//! [`SessionFilter`]: keywiz_stats::SessionFilter

/// Time slice over which a stats page is scoped. Cycles through
/// Alt+↑/↓.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Granularity {
    /// Only the session that's currently running. P1 default.
    CurrentSession,
    /// One calendar day. `offset = 0` = today.
    Day,
    /// Rolling 7-day window ending at the chosen offset.
    Week,
    /// One calendar month. `offset = 0` = this month.
    Month,
    /// One calendar year. `offset = 0` = this year.
    Year,
    /// No time bound — the entire event stream for the (layout,
    /// keyboard) scope.
    All,
}

impl Granularity {
    /// Alt+↓ order — session → day → week → month → year → all
    /// → session.
    pub fn next(self) -> Self {
        match self {
            Self::CurrentSession => Self::Day,
            Self::Day => Self::Week,
            Self::Week => Self::Month,
            Self::Month => Self::Year,
            Self::Year => Self::All,
            Self::All => Self::CurrentSession,
        }
    }

    /// Alt+↑ order — reverse of [`Self::next`].
    pub fn prev(self) -> Self {
        match self {
            Self::CurrentSession => Self::All,
            Self::Day => Self::CurrentSession,
            Self::Week => Self::Day,
            Self::Month => Self::Week,
            Self::Year => Self::Month,
            Self::All => Self::Year,
        }
    }
}

/// A (layout_name, keyboard_name) pair — the atomic unit of stats
/// filtering. Keyboards and layouts are semantically linked: a
/// drifter-on-Halcyon session is different from a drifter-on-Ortho
/// session, and "halcyon across all layouts" isn't a meaningful
/// question. So the filter cycles through recorded *combinations*
/// rather than independent layout/keyboard axes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Combo {
    pub layout: String,
    pub keyboard: String,
}

/// Default + min + max number of buckets P2 Progression shows.
/// The range axis on P2 lets Alt+←/→ shrink/expand the window
/// rather than walk offsets. Bounded so the table always fits on
/// a reasonable screen and has at least two points to compare.
pub const RANGE_DEFAULT: usize = 7;
pub const RANGE_MIN: usize = 2;
pub const RANGE_MAX: usize = 30;

/// Active filter state for the F4 stats modal.
#[derive(Debug, Clone)]
pub struct StatsFilter {
    /// Restrict to sessions that ran on this specific (layout,
    /// keyboard) combination. `None` = no scope restriction (every
    /// recorded combo). Matches by name, not hash — iterations of
    /// the same-named layout stay grouped.
    pub combo: Option<Combo>,
    /// Time slice the page is scoped to.
    pub granularity: Granularity,
    /// Position within the granularity. `0` = current/most-recent
    /// bucket; `-1` = one bucket back, etc. Unused when granularity
    /// is `CurrentSession` or `All`.
    pub offset: i64,
    /// Number of buckets the P2 progression page shows. P1 and P3
    /// ignore this (they read a single bucket defined by `offset`).
    pub range: usize,
}

impl Default for StatsFilter {
    fn default() -> Self {
        Self {
            combo: None,
            granularity: Granularity::CurrentSession,
            offset: 0,
            range: RANGE_DEFAULT,
        }
    }
}

impl StatsFilter {
    /// Advance granularity; reset offset because absolute positions
    /// don't translate across granularities (`offset=-3` means "three
    /// days ago" at Day but "three months ago" at Month).
    pub fn next_granularity(&mut self) {
        self.granularity = self.granularity.next();
        self.offset = 0;
    }

    /// Back one step through the granularity cycle.
    pub fn prev_granularity(&mut self) {
        self.granularity = self.granularity.prev();
        self.offset = 0;
    }

    /// Walk back one bucket (Alt+→ on P1/P3 — older data).
    pub fn older_offset(&mut self) {
        self.offset -= 1;
    }

    /// Walk forward one bucket (Alt+← on P1/P3 — newer data, up
    /// to 0).
    pub fn newer_offset(&mut self) {
        if self.offset < 0 {
            self.offset += 1;
        }
    }

    /// Widen the P2 progression range by one bucket (Alt+→ on P2).
    pub fn wider_range(&mut self) {
        if self.range < RANGE_MAX {
            self.range += 1;
        }
    }

    /// Narrow the P2 progression range by one bucket (Alt+← on P2).
    pub fn narrower_range(&mut self) {
        if self.range > RANGE_MIN {
            self.range -= 1;
        }
    }
}
