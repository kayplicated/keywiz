//! In-memory [`EventStore`] — reference implementation for tests
//! and opt-out-of-persistence sessions.
//!
//! Every write is O(1); every read is a linear scan with filter
//! predicates. Not optimized — this isn't the production path,
//! it's the spec. The SQLite store must agree with this one on
//! every contract test.

use std::collections::HashMap;

use anyhow::{Result, anyhow};

use super::{EventFilter, EventStore, SessionFilter};
use crate::event::Event;
use crate::session::{SessionId, SessionSummary};
use crate::snapshot::{KeyboardHash, KeyboardSnapshot, LayoutHash, LayoutSnapshot};

/// In-memory store. Holds everything in `Vec`s and `HashMap`s.
#[derive(Default)]
pub struct MemoryStore {
    next_session_id: i64,
    events: Vec<Event>,
    sessions: HashMap<SessionId, SessionSummary>,
    layout_snapshots: HashMap<LayoutHash, LayoutSnapshot>,
    keyboard_snapshots: HashMap<KeyboardHash, KeyboardSnapshot>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl EventStore for MemoryStore {
    fn begin_session(
        &mut self,
        layout: &LayoutSnapshot,
        keyboard: &KeyboardSnapshot,
        exercise_category: &str,
        exercise_instance: Option<&str>,
        started_at_ms: i64,
    ) -> Result<SessionId> {
        // Snapshot upsert — first-seen wins, later writes are no-ops.
        self.layout_snapshots
            .entry(layout.hash.clone())
            .or_insert_with(|| layout.clone());
        self.keyboard_snapshots
            .entry(keyboard.hash.clone())
            .or_insert_with(|| keyboard.clone());

        self.next_session_id += 1;
        let id = SessionId(self.next_session_id);
        let summary = SessionSummary {
            session_id: id,
            started_at_ms,
            ended_at_ms: None,
            layout_hash: layout.hash.clone(),
            layout_name: layout.name.clone(),
            keyboard_hash: keyboard.hash.clone(),
            keyboard_name: keyboard.name.clone(),
            exercise_category: exercise_category.to_string(),
            exercise_instance: exercise_instance.map(str::to_string),
            total_events: 0,
            total_correct: 0,
        };
        self.sessions.insert(id, summary);
        Ok(id)
    }

    fn end_session(
        &mut self,
        session_id: SessionId,
        ended_at_ms: i64,
    ) -> Result<SessionSummary> {
        let summary = self
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| anyhow!("end_session: unknown session {session_id}"))?;
        summary.ended_at_ms = Some(ended_at_ms);
        Ok(summary.clone())
    }

    fn record(&mut self, event: &Event) -> Result<()> {
        let summary = self
            .sessions
            .get_mut(&event.session_id)
            .ok_or_else(|| anyhow!("record: unknown session {}", event.session_id))?;
        summary.total_events += 1;
        if event.correct {
            summary.total_correct += 1;
        }
        self.events.push(event.clone());
        Ok(())
    }

    fn events<'a>(
        &'a self,
        filter: &EventFilter,
    ) -> Result<Box<dyn Iterator<Item = Result<Event>> + 'a>> {
        // Clone the filter fields so the iterator doesn't borrow.
        let f = filter.clone();
        // Join-through-session for layout/keyboard/category predicates.
        let sessions = self.sessions.clone();
        let iter = self
            .events
            .iter()
            .filter(move |ev| matches_event(ev, &f, &sessions))
            .cloned()
            .map(Ok);
        Ok(Box::new(iter))
    }

    fn sessions(&self, filter: &SessionFilter) -> Result<Vec<SessionSummary>> {
        let mut rows: Vec<SessionSummary> = self
            .sessions
            .values()
            .filter(|s| matches_session(s, filter))
            .cloned()
            .collect();
        rows.sort_by(|a, b| b.started_at_ms.cmp(&a.started_at_ms));
        if let Some(limit) = filter.limit {
            rows.truncate(limit);
        }
        Ok(rows)
    }

    fn layout_snapshot(&self, hash: &LayoutHash) -> Result<Option<LayoutSnapshot>> {
        Ok(self.layout_snapshots.get(hash).cloned())
    }

    fn keyboard_snapshot(&self, hash: &KeyboardHash) -> Result<Option<KeyboardSnapshot>> {
        Ok(self.keyboard_snapshots.get(hash).cloned())
    }
}

fn matches_event(
    ev: &Event,
    f: &EventFilter,
    sessions: &HashMap<SessionId, SessionSummary>,
) -> bool {
    if let Some(id) = f.session_id
        && ev.session_id != id
    {
        return false;
    }
    if let Some(ids) = &f.session_ids
        && (ids.is_empty() || !ids.contains(&ev.session_id))
    {
        return false;
    }
    if let Some(correct) = f.correct
        && ev.correct != correct
    {
        return false;
    }
    if let Some(from) = f.from_ms
        && ev.ts_ms < from
    {
        return false;
    }
    if let Some(until) = f.until_ms
        && ev.ts_ms > until
    {
        return false;
    }
    // Per-session predicates — join through the sessions table.
    if f.layout_hash.is_some()
        || f.keyboard_hash.is_some()
        || f.exercise_category.is_some()
        || f.exercise_categories.is_some()
    {
        let Some(session) = sessions.get(&ev.session_id) else {
            return false;
        };
        if let Some(h) = &f.layout_hash
            && &session.layout_hash != h
        {
            return false;
        }
        if let Some(h) = &f.keyboard_hash
            && &session.keyboard_hash != h
        {
            return false;
        }
        if let Some(cat) = &f.exercise_category
            && &session.exercise_category != cat
        {
            return false;
        }
        if let Some(cats) = &f.exercise_categories
            && !cats.iter().any(|c| c == &session.exercise_category)
        {
            return false;
        }
    }
    true
}

fn matches_session(s: &SessionSummary, f: &SessionFilter) -> bool {
    if let Some(h) = &f.layout_hash
        && &s.layout_hash != h
    {
        return false;
    }
    if let Some(n) = &f.layout_name
        && &s.layout_name != n
    {
        return false;
    }
    if let Some(h) = &f.keyboard_hash
        && &s.keyboard_hash != h
    {
        return false;
    }
    if let Some(cat) = &f.exercise_category
        && &s.exercise_category != cat
    {
        return false;
    }
    if let Some(from) = f.from_ms
        && s.started_at_ms < from
    {
        return false;
    }
    if let Some(until) = f.until_ms
        && s.started_at_ms > until
    {
        return false;
    }
    true
}
