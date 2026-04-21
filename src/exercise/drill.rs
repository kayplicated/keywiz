//! Drill exercise — single character at a time with adaptive
//! level progression.
//!
//! Two loops cooperate, both driven from `advance`:
//!
//! 1. **Autoscaler** — a rolling-window accuracy gate that moves
//!    the drill between three levels (home / home+top / all). At a
//!    **hot** level (any letter at or above [`HOT_HEAT_THRESHOLD`]),
//!    the promote threshold is strict ([`PROMOTION_HOT_PCT`]) and
//!    the minimum-keys grace period is long ([`MIN_KEYS_HOT`]). At
//!    a **cold** level (all letters below the threshold), the
//!    promote threshold is lenient ([`PROMOTION_COLD_PCT`]) and the
//!    grace period is short ([`MIN_KEYS_COLD`]) — so returning
//!    practice blows through already-mastered ground fast.
//!    Demotion uses one threshold regardless of heat: if you're
//!    tanking, you're tanking.
//!
//! 2. **Heat-weighted picker** — within the current level, each
//!    letter's pick weight is `baseline + heat`. Cold letters still
//!    appear (baseline), hot letters appear proportionally more.
//!    The autoscaler decides *which level*; the picker decides
//!    *which letter at that level*. They never fight — the picker
//!    only sees letters the autoscaler has currently unlocked.
//!
//! Level is always inferred from current stats at start time, so
//! no "last level" is persisted across sessions. The heatmap is
//! the source of truth; if your persistent stats show home row is
//! clean, the drill will autoscale past home row in a few
//! keystrokes even on a cold launch.

use std::collections::VecDeque;

use rand::distr::weighted::WeightedIndex;
use rand::distr::Distribution;
use rand::prelude::IndexedRandom;

use crate::engine::placement::DisplayState;
use crate::exercise::Exercise;
use crate::stats::Stats;

// ---- tuning ----

/// Rolling-window size for accuracy tracking. ~20 keystrokes = ~one
/// word's worth of practice.
const WINDOW_SIZE: usize = 20;

/// Promotion threshold (%) when the current level has hot letters.
/// Requires earned accuracy before unlocking the next level.
const PROMOTION_HOT_PCT: f64 = 90.0;

/// Promotion threshold (%) when the current level has no hot
/// letters. ~15–20% faster than the hot threshold, so returning
/// practice breezes through clean levels.
const PROMOTION_COLD_PCT: f64 = 75.0;

/// Demotion threshold (%). Independent of heat — bad typing is bad
/// typing.
const DEMOTION_PCT: f64 = 70.0;

/// Min keystrokes at a level before any promotion/demotion check,
/// when the level is hot. Prevents the autoscaler from bouncing on
/// the first few keystrokes of fresh practice.
const MIN_KEYS_HOT: usize = 30;

/// Min keystrokes at a level before any promotion check, when the
/// level is cold. Short so clean levels don't feel like a speedbump.
const MIN_KEYS_COLD: usize = 10;

/// Heat level at or above which a letter counts as "hot" (i.e.
/// still outside the cool-blue range of the heatmap gradient).
/// Tuned so lingering residual heat from one stray miss doesn't pin
/// the drill on a low level forever.
const HOT_HEAT_THRESHOLD: u32 = 5;

/// Baseline weight in the heat-weighted picker. Cold letters always
/// get this weight; hot letters get `baseline + heat_boost(heat)`.
const BASELINE_WEIGHT: f64 = 1.0;

/// Exponential base divisor for the heat boost curve. The picker's
/// boost for a letter at heat `h` is `2^(h / HEAT_BOOST_DIVISOR) - 1`.
/// Smaller = more aggressive at low heat; larger = ignores small
/// problems and only escalates on genuinely contested keys.
///
/// At divisor 4:
///   heat 1 → 1.2× cold (noise-level, fresh stray misses don't
///            dominate picking)
///   heat 5 → 2.4× cold (out of the blue range, real but moderate)
///   heat 10 → 5.7× cold (clearly a problem)
///   heat 20 → 32× cold (maxed-out key, dominates picking)
const HEAT_BOOST_DIVISOR: f64 = 4.0;

/// Per-letter pick weight given its heat. `heat_boost(0) = 0`, so
/// cold letters fall back to baseline.
fn heat_boost(heat: u32) -> f64 {
    2f64.powf(heat as f64 / HEAT_BOOST_DIVISOR) - 1.0
}

/// Size multiplier for the recent-picks window used by the fairness
/// boost. Window holds `RECENT_WINDOW_MULT * candidates.len()`
/// picks; larger = smoother fairness tracking, smaller = more
/// reactive to recent under-service.
const RECENT_WINDOW_MULT: usize = 2;

/// How hard the fairness boost pushes under-served letters.
///
/// A letter picked at its exact fair share gets +0. A letter with
/// zero picks in the window gets `FAIRNESS_GAIN` added to its
/// weight. Persistently-under-served letters scale further:
/// see [`fairness_boost`] for the quadratic shortfall curve that
/// keeps fairness loud enough to out-scream heat when a cold
/// letter has been neglected across many picks.
const FAIRNESS_GAIN: f64 = 5.0;

/// Compute how much to boost a candidate's weight for fairness,
/// given how often it's been picked in the recent window. Under-
/// served letters get a positive boost; at-or-over-served letters
/// get 0.
///
/// The shortfall is squared before being scaled by `FAIRNESS_GAIN`.
/// This keeps mildly under-served letters from being spammed (they
/// only get a gentle nudge) while making deeply-neglected letters
/// genuinely loud — a letter at 0% of fair share screams much
/// harder than one at 50%, which matters when heat is pushing a
/// few letters far above their share and the remainder has to
/// compete for what's left.
fn fairness_boost(count: usize, total: usize, candidate_count: usize) -> f64 {
    if total == 0 || candidate_count == 0 {
        return 0.0;
    }
    let fair_share = 1.0 / candidate_count as f64;
    let actual = count as f64 / total as f64;
    let shortfall_ratio = ((fair_share - actual) / fair_share).max(0.0);
    shortfall_ratio * shortfall_ratio * FAIRNESS_GAIN
}

// ---- types ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrillLevel {
    HomeRow,
    HomeAndTop,
    AllRows,
}

impl DrillLevel {
    pub fn label(self) -> &'static str {
        match self {
            DrillLevel::HomeRow => "Home Row",
            DrillLevel::HomeAndTop => "Home + Top Row",
            DrillLevel::AllRows => "All Rows",
        }
    }

    fn index(self) -> usize {
        match self {
            DrillLevel::HomeRow => 0,
            DrillLevel::HomeAndTop => 1,
            DrillLevel::AllRows => 2,
        }
    }

    fn from_index(i: usize) -> Self {
        match i {
            0 => DrillLevel::HomeRow,
            1 => DrillLevel::HomeAndTop,
            _ => DrillLevel::AllRows,
        }
    }

    fn next(self) -> Option<DrillLevel> {
        match self {
            DrillLevel::HomeRow => Some(DrillLevel::HomeAndTop),
            DrillLevel::HomeAndTop => Some(DrillLevel::AllRows),
            DrillLevel::AllRows => None,
        }
    }

    fn prev(self) -> Option<DrillLevel> {
        match self {
            DrillLevel::HomeRow => None,
            DrillLevel::HomeAndTop => Some(DrillLevel::HomeRow),
            DrillLevel::AllRows => Some(DrillLevel::HomeAndTop),
        }
    }
}

pub struct DrillExercise {
    chars_by_level: [Vec<char>; 3],
    level: DrillLevel,
    current: char,
    streak: u32,
    window: VecDeque<bool>,
    keys_at_level: usize,
    /// Recently-served chars, for the picker's fairness boost. The
    /// ring's capacity is `RECENT_WINDOW_MULT * current_chars.len()`
    /// and is resized on level change.
    recent_picks: VecDeque<char>,
}

impl DrillExercise {
    /// Build a drill against the pre-computed per-level char sets,
    /// with the starting level inferred from `stats`.
    pub fn new(chars_by_level: [Vec<char>; 3], stats: &Stats) -> Self {
        let level = pick_starting_level(&chars_by_level, stats);
        let chars = &chars_by_level[level.index()];
        let recent_cap = RECENT_WINDOW_MULT * chars.len().max(1);
        let recent_picks = VecDeque::with_capacity(recent_cap);
        let current = pick_weighted(chars, stats, None, &recent_picks).unwrap_or('a');
        let mut drill = DrillExercise {
            chars_by_level,
            level,
            current,
            streak: 0,
            window: VecDeque::with_capacity(WINDOW_SIZE),
            keys_at_level: 0,
            recent_picks,
        };
        drill.record_pick(current);
        drill
    }

    fn record_pick(&mut self, ch: char) {
        let cap = RECENT_WINDOW_MULT * self.current_chars().len().max(1);
        if self.recent_picks.len() >= cap {
            self.recent_picks.pop_front();
        }
        self.recent_picks.push_back(ch);
    }

    fn current_chars(&self) -> &[char] {
        &self.chars_by_level[self.level.index()]
    }

    fn window_accuracy_pct(&self) -> f64 {
        if self.window.is_empty() {
            return 100.0;
        }
        let correct = self.window.iter().filter(|&&b| b).count();
        (correct as f64 / self.window.len() as f64) * 100.0
    }

    fn level_is_hot(&self, stats: &Stats) -> bool {
        level_is_hot(self.current_chars(), stats)
    }

    fn reset_progression(&mut self) {
        self.window.clear();
        self.keys_at_level = 0;
        // Fairness tracking resets on level change — the candidate
        // set changed, so prior-level counts don't make sense.
        self.recent_picks.clear();
    }

    // TODO: the promote / demote branches below share most of
    // their body (reset progression, repick, record). They diverge
    // in one spot — demote keeps the current letter when it's still
    // in the smaller pool, promote always repicks — which is enough
    // to make consolidation awkward without helper naming gymnastics.
    // Leaving split for now; revisit if a third branch ever appears
    // or the semantics drift further.
    fn try_autoscale(&mut self, stats: &Stats) {
        let hot = self.level_is_hot(stats);
        let min_keys = if hot { MIN_KEYS_HOT } else { MIN_KEYS_COLD };
        if self.keys_at_level < min_keys || self.window.len() < WINDOW_SIZE {
            return;
        }
        let acc = self.window_accuracy_pct();
        let promote = if hot {
            PROMOTION_HOT_PCT
        } else {
            PROMOTION_COLD_PCT
        };
        if acc >= promote {
            if let Some(next) = self.level.next() {
                self.level = next;
                self.reset_progression();
                // Promotion means the new level includes fresh letters
                // the user hasn't been practicing — repick to surface
                // one of them.
                self.current =
                    pick_weighted(self.current_chars(), stats, None, &self.recent_picks)
                        .unwrap_or(self.current);
                self.record_pick(self.current);
            }
        } else if acc < DEMOTION_PCT {
            if let Some(prev) = self.level.prev() {
                self.level = prev;
                self.reset_progression();
                // Demotion: if the user was mid-fight with a letter
                // that still exists in the smaller level, keep that
                // letter as the target. Otherwise the letter they
                // were about to finally get right would vanish and
                // feel like the app gave up on them. Only repick
                // when the current letter is no longer in scope.
                if !self.current_chars().contains(&self.current) {
                    self.current =
                        pick_weighted(self.current_chars(), stats, None, &self.recent_picks)
                            .unwrap_or(self.current);
                    self.record_pick(self.current);
                }
            }
        }
    }
}

impl Exercise for DrillExercise {
    fn name(&self) -> &str {
        "drill"
    }

    fn short(&self) -> &str {
        "Drill"
    }

    fn expected(&self) -> Option<char> {
        Some(self.current)
    }

    fn advance(&mut self, stats: &Stats, correct: bool) {
        self.keys_at_level += 1;
        if self.window.len() >= WINDOW_SIZE {
            self.window.pop_front();
        }
        self.window.push_back(correct);

        if correct {
            self.streak += 1;
        } else {
            self.streak = 0;
        }

        // Autoscale first — it may switch the level (and the picker
        // will then sample from the new level's chars).
        self.try_autoscale(stats);

        if correct {
            self.current =
                pick_weighted(self.current_chars(), stats, Some(self.current), &self.recent_picks)
                    .unwrap_or(self.current);
            self.record_pick(self.current);
        }
    }

    fn is_done(&self) -> bool {
        false
    }

    fn fill_display(&self, display: &mut DisplayState) {
        display.drill_current_char = Some(self.current);
        display.drill_level = Some(self.level.label().to_string());
        display.drill_streak = Some(self.streak);
        display.highlight_char = Some(self.current);
    }
}

// ---- helpers ----

/// Pick the lowest level that still has hot letters. "Lowest" matters:
/// if home row has trouble and so does all-rows, we go to home row —
/// fix the foundation first. If nothing is hot anywhere, start at
/// all-rows for broad practice.
fn pick_starting_level(chars_by_level: &[Vec<char>; 3], stats: &Stats) -> DrillLevel {
    for i in 0..3 {
        if level_is_hot(&chars_by_level[i], stats) {
            return DrillLevel::from_index(i);
        }
    }
    DrillLevel::AllRows
}

fn level_is_hot(chars: &[char], stats: &Stats) -> bool {
    chars.iter().any(|c| {
        stats
            .get(*c)
            .map(|r| r.heat >= HOT_HEAT_THRESHOLD)
            .unwrap_or(false)
    })
}

/// Pick one char from `chars`, weighted by `baseline + heat_boost +
/// fairness_boost`. `avoid` suppresses immediate repeats (the UI
/// doesn't indicate a same-key re-presentation, so repeats read as
/// input glitches). `recent_picks` is the ring buffer that drives
/// the fairness correction — letters under-served in the window
/// get bumped toward their fair share.
fn pick_weighted(
    chars: &[char],
    stats: &Stats,
    avoid: Option<char>,
    recent_picks: &VecDeque<char>,
) -> Option<char> {
    if chars.is_empty() {
        return None;
    }
    // Try to exclude the avoid char; fall back if it's the only option.
    let candidates: Vec<char> = match avoid {
        Some(skip) if chars.len() > 1 => chars.iter().copied().filter(|c| *c != skip).collect(),
        _ => chars.to_vec(),
    };
    if candidates.is_empty() {
        return chars.choose(&mut rand::rng()).copied();
    }

    let total = recent_picks.len();
    let n = candidates.len();
    let weights: Vec<f64> = candidates
        .iter()
        .map(|c| {
            let heat = stats.get(*c).map(|r| r.heat).unwrap_or(0);
            let count = recent_picks.iter().filter(|&&p| p == *c).count();
            BASELINE_WEIGHT + heat_boost(heat) + fairness_boost(count, total, n)
        })
        .collect();

    let dist = match WeightedIndex::new(&weights) {
        Ok(d) => d,
        Err(_) => return candidates.choose(&mut rand::rng()).copied(),
    };
    Some(candidates[dist.sample(&mut rand::rng())])
}
