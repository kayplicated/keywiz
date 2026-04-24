//! Event-stream keystroke statistics for keywiz.
//!
//! # Shape
//!
//! One struct per keystroke ([`Event`]) — the source of truth.
//! Session-level rollups ([`SessionSummary`]) are written on
//! session close so "last N sessions" queries don't have to
//! aggregate the event stream. Sessions reference the layout and
//! keyboard they ran against by *content hash* (see
//! [`snapshot`] and [`hash`]), so "drifter after the k/h swap"
//! and "drifter before the k/h swap" are unambiguously different
//! iterations even if the filename never changed.
//!
//! Storage is abstracted behind [`EventStore`] — views depend on
//! the trait, not any concrete backing. Two impls are planned:
//! `store::sqlite` (production) and `store::memory` (tests /
//! opt-out-of-persistence).
//!
//! # Engine surface
//!
//! The engine talks to [`Stats`] only. It calls
//! [`Stats::begin_session`] on exercise/layout/keyboard change,
//! [`Stats::record`] on every keystroke, and
//! [`Stats::end_session`] on shutdown. Views read through
//! [`Stats::store`] using [`EventFilter`] and [`SessionFilter`].
//!
//! # Views
//!
//! Intentionally absent from this crate's initial scaffold. Each
//! view lands as its own module under `views/` — per-key heat,
//! WPM, session summaries, bigram miss rates, drift cross-reference,
//! and so on. Views are free functions over the store, not trait
//! implementations — uniformity for its own sake would cost more
//! than it buys. A `trait View` can be introduced later if a
//! "run every view" use case appears.

pub mod event;
pub mod facade;
pub mod hash;
pub mod session;
pub mod snapshot;
pub mod store;
pub mod views;

pub use event::Event;
pub use facade::{IDLE_THRESHOLD_MS, Stats};
pub use hash::{Canonical, canonical_json, hash_canonical, keyboard_hash, layout_hash};
pub use session::{SessionId, SessionSummary};
pub use snapshot::{KeyboardHash, KeyboardSnapshot, LayoutHash, LayoutSnapshot};
pub use store::{EventFilter, EventStore, SessionFilter};
