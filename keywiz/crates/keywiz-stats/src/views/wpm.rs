//! WPM / APM view — derives typing-speed numbers from the event
//! stream's inter-keystroke timings.
//!
//! Two concepts keywiz treats seriously:
//!
//! - **Active time** is the sum of `Event::delta_ms` across a
//!   session, *not* the wall-clock span from first event to last.
//!   Typing pauses longer than [`IDLE_THRESHOLD_MS`] collapse
//!   `delta_ms` to `None` at record time, so the active-time sum
//!   excludes AFK stretches automatically.
//!
//!   [`IDLE_THRESHOLD_MS`]: crate::IDLE_THRESHOLD_MS
//!
//! - **"Word" = 5 characters**, the industry-standard convention
//!   (Monkeytype, typingtest.com, every benchmark site). Actual
//!   word-length varies too much to be useful for cross-session
//!   comparison.
//!
//! **Net vs gross WPM:** net counts only correct keystrokes; gross
//! counts all. Net is the honest "usable output" number; gross
//! reflects raw hand speed even when fingers are outrunning the
//! brain. Both are exposed; callers pick what to display.

use anyhow::Result;

use crate::session::SessionId;
use crate::{EventFilter, EventStore};

/// Chars per "word" for WPM computation.
const CHARS_PER_WORD: f64 = 5.0;

/// Millis per minute.
const MS_PER_MINUTE: f64 = 60_000.0;

/// Typing-speed aggregates for a session.
#[derive(Debug, Clone, Copy, Default)]
pub struct SessionWpm {
    /// Sum of `delta_ms` across all events in the session. Excludes
    /// the first event (no predecessor) and any gaps past the idle
    /// threshold. This is the "time actually typing" figure.
    pub active_ms: u64,
    /// Every recorded keystroke, correct or not.
    pub total_keystrokes: u64,
    /// Correct keystrokes only.
    pub correct_keystrokes: u64,
}

impl SessionWpm {
    /// Net WPM: correct-chars / 5 / minutes-of-active-time.
    /// Returns 0 on an empty session. Net penalizes misses by
    /// excluding them from the numerator — this is the figure
    /// that tracks "usable typing speed."
    pub fn net_wpm(&self) -> f64 {
        if self.active_ms == 0 {
            return 0.0;
        }
        let minutes = self.active_ms as f64 / MS_PER_MINUTE;
        (self.correct_keystrokes as f64 / CHARS_PER_WORD) / minutes
    }

    /// Gross WPM: all-chars / 5 / minutes. Hand speed regardless
    /// of accuracy. Always ≥ `net_wpm`.
    pub fn gross_wpm(&self) -> f64 {
        if self.active_ms == 0 {
            return 0.0;
        }
        let minutes = self.active_ms as f64 / MS_PER_MINUTE;
        (self.total_keystrokes as f64 / CHARS_PER_WORD) / minutes
    }

    /// Actions per minute: raw keystrokes per minute of active time.
    /// APM is WPM without the "word = 5 chars" translation.
    pub fn apm(&self) -> f64 {
        if self.active_ms == 0 {
            return 0.0;
        }
        let minutes = self.active_ms as f64 / MS_PER_MINUTE;
        self.total_keystrokes as f64 / minutes
    }
}

/// Tally typing-speed numbers for `session_id`. Returns an empty
/// `SessionWpm` if the session has no events — typing-speed views
/// treat "no data" and "no session" the same.
pub fn live_for(store: &dyn EventStore, session_id: SessionId) -> Result<SessionWpm> {
    let filter = EventFilter {
        session_id: Some(session_id),
        ..Default::default()
    };
    let mut wpm = SessionWpm::default();
    for event in store.events(&filter)? {
        let event = event?;
        wpm.total_keystrokes += 1;
        if event.correct {
            wpm.correct_keystrokes += 1;
        }
        if let Some(ms) = event.delta_ms {
            wpm.active_ms += ms as u64;
        }
    }
    Ok(wpm)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Zero-event sessions must not divide by zero. Every WPM method
    /// returns 0 cleanly.
    #[test]
    fn empty_session_returns_zero() {
        let wpm = SessionWpm::default();
        assert_eq!(wpm.net_wpm(), 0.0);
        assert_eq!(wpm.gross_wpm(), 0.0);
        assert_eq!(wpm.apm(), 0.0);
    }

    /// 100 correct keystrokes in 60_000 ms (1 minute) of active
    /// time = 100/5 = 20 net WPM, 100 APM.
    #[test]
    fn canonical_minute_of_typing() {
        let wpm = SessionWpm {
            active_ms: 60_000,
            total_keystrokes: 100,
            correct_keystrokes: 100,
        };
        assert_eq!(wpm.net_wpm(), 20.0);
        assert_eq!(wpm.gross_wpm(), 20.0);
        assert_eq!(wpm.apm(), 100.0);
    }

    /// Net penalizes misses, gross does not. 80 correct + 20
    /// wrong in a minute = 16 net, 20 gross.
    #[test]
    fn net_below_gross_when_missing() {
        let wpm = SessionWpm {
            active_ms: 60_000,
            total_keystrokes: 100,
            correct_keystrokes: 80,
        };
        assert_eq!(wpm.net_wpm(), 16.0);
        assert_eq!(wpm.gross_wpm(), 20.0);
    }

    /// Half-minute of typing scales proportionally.
    #[test]
    fn partial_minute_scales() {
        let wpm = SessionWpm {
            active_ms: 30_000,
            total_keystrokes: 50,
            correct_keystrokes: 50,
        };
        assert_eq!(wpm.net_wpm(), 20.0);
    }
}
