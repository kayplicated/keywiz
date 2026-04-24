//! SQLite-backed [`EventStore`] — the production path.
//!
//! Single file at a caller-supplied path. Opens existing or creates
//! fresh, applies schema. Held behind a `Mutex` so the trait's
//! `Send + Sync` bound holds despite `rusqlite::Connection` being
//! `!Sync`. Keywiz is single-process and single-writer, so lock
//! contention is nil.
//!
//! Writes are one-INSERT-per-event — no batching. At typing speeds
//! (~20 keystrokes/sec peak) each INSERT is well under a millisecond,
//! so we stay well inside the frame budget. Revisit if the render
//! loop ever starts stuttering.

use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, OptionalExtension, params};

use super::{EventFilter, EventStore, SessionFilter};
use crate::event::Event;
use crate::session::{SessionId, SessionSummary};
use crate::snapshot::{KeyboardHash, KeyboardSnapshot, LayoutHash, LayoutSnapshot};

/// Current schema version. Bump alongside a migration in
/// [`SqliteStore::apply_schema`] when the shape changes.
const SCHEMA_VERSION: i64 = 1;

/// SQLite-backed store.
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    /// Open a store at `path`. Creates the file and applies the
    /// schema if it doesn't exist; opens cleanly if it does.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("opening sqlite store at {}", path.display()))?;
        let store = Self { conn: Mutex::new(conn) };
        store.apply_schema()?;
        Ok(store)
    }

    /// Open an ephemeral in-memory store — for tests.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("opening in-memory sqlite store")?;
        let store = Self { conn: Mutex::new(conn) };
        store.apply_schema()?;
        Ok(store)
    }

    fn apply_schema(&self) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY);

            CREATE TABLE IF NOT EXISTS layout_snapshots (
                hash TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                canonical_json TEXT NOT NULL,
                first_seen_ms INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS keyboard_snapshots (
                hash TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                canonical_json TEXT NOT NULL,
                first_seen_ms INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sessions (
                session_id INTEGER PRIMARY KEY AUTOINCREMENT,
                started_at_ms INTEGER NOT NULL,
                ended_at_ms INTEGER,
                layout_hash TEXT NOT NULL REFERENCES layout_snapshots(hash),
                layout_name TEXT NOT NULL,
                keyboard_hash TEXT NOT NULL REFERENCES keyboard_snapshots(hash),
                keyboard_name TEXT NOT NULL,
                exercise_category TEXT NOT NULL,
                exercise_instance TEXT,
                total_events INTEGER NOT NULL DEFAULT 0,
                total_correct INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS sessions_by_layout
                ON sessions(layout_hash, started_at_ms DESC);
            CREATE INDEX IF NOT EXISTS sessions_by_started
                ON sessions(started_at_ms DESC);

            CREATE TABLE IF NOT EXISTS events (
                session_id INTEGER NOT NULL REFERENCES sessions(session_id),
                ts_ms INTEGER NOT NULL,
                expected TEXT NOT NULL,
                typed TEXT NOT NULL,
                correct INTEGER NOT NULL,
                delta_ms INTEGER
            );
            CREATE INDEX IF NOT EXISTS events_by_session
                ON events(session_id, ts_ms);
            "#,
        )?;

        // Stamp or verify schema_version.
        let existing: Option<i64> = conn
            .query_row("SELECT version FROM schema_version LIMIT 1", [], |r| {
                r.get(0)
            })
            .optional()?;
        match existing {
            Some(v) if v == SCHEMA_VERSION => {}
            Some(v) => {
                return Err(anyhow!(
                    "sqlite store has schema version {v}, expected {SCHEMA_VERSION}. \
                     Schema migrations not yet implemented."
                ));
            }
            None => {
                conn.execute(
                    "INSERT INTO schema_version (version) VALUES (?)",
                    params![SCHEMA_VERSION],
                )?;
            }
        }
        Ok(())
    }
}

impl EventStore for SqliteStore {
    fn begin_session(
        &mut self,
        layout: &LayoutSnapshot,
        keyboard: &KeyboardSnapshot,
        exercise_category: &str,
        exercise_instance: Option<&str>,
        started_at_ms: i64,
    ) -> Result<SessionId> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");

        // Snapshot upserts — INSERT OR IGNORE keeps first-seen row.
        conn.execute(
            "INSERT OR IGNORE INTO layout_snapshots \
             (hash, name, canonical_json, first_seen_ms) VALUES (?, ?, ?, ?)",
            params![
                &layout.hash.0,
                &layout.name,
                &layout.canonical_json,
                layout.first_seen_ms,
            ],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO keyboard_snapshots \
             (hash, name, canonical_json, first_seen_ms) VALUES (?, ?, ?, ?)",
            params![
                &keyboard.hash.0,
                &keyboard.name,
                &keyboard.canonical_json,
                keyboard.first_seen_ms,
            ],
        )?;

        conn.execute(
            "INSERT INTO sessions \
             (started_at_ms, layout_hash, layout_name, keyboard_hash, \
              keyboard_name, exercise_category, exercise_instance) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![
                started_at_ms,
                &layout.hash.0,
                &layout.name,
                &keyboard.hash.0,
                &keyboard.name,
                exercise_category,
                exercise_instance,
            ],
        )?;
        Ok(SessionId(conn.last_insert_rowid()))
    }

    fn end_session(
        &mut self,
        session_id: SessionId,
        ended_at_ms: i64,
    ) -> Result<SessionSummary> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let updated = conn.execute(
            "UPDATE sessions SET ended_at_ms = ? WHERE session_id = ?",
            params![ended_at_ms, session_id.0],
        )?;
        if updated == 0 {
            return Err(anyhow!("end_session: unknown session {session_id}"));
        }
        row_to_summary(&conn, session_id)
    }

    fn record(&mut self, event: &Event) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        // Single transaction keeps events table + session counters
        // in lockstep without a race.
        let tx = conn.unchecked_transaction()?;
        let exists: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE session_id = ?",
                params![event.session_id.0],
                |r| r.get(0),
            )?;
        if exists == 0 {
            return Err(anyhow!(
                "record: unknown session {}",
                event.session_id
            ));
        }
        tx.execute(
            "INSERT INTO events (session_id, ts_ms, expected, typed, correct, delta_ms) \
             VALUES (?, ?, ?, ?, ?, ?)",
            params![
                event.session_id.0,
                event.ts_ms,
                event.expected.to_string(),
                event.typed.to_string(),
                event.correct as i64,
                event.delta_ms,
            ],
        )?;
        tx.execute(
            "UPDATE sessions SET total_events = total_events + 1, \
                 total_correct = total_correct + ? \
                 WHERE session_id = ?",
            params![event.correct as i64, event.session_id.0],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn events<'a>(
        &'a self,
        filter: &EventFilter,
    ) -> Result<Box<dyn Iterator<Item = Result<Event>> + 'a>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");

        // Build dynamic WHERE. Bind params as strings/ints per type.
        let mut clauses: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(id) = filter.session_id {
            clauses.push("e.session_id = ?".into());
            params.push(Box::new(id.0));
        }
        if let Some(ids) = &filter.session_ids {
            if ids.is_empty() {
                // Empty set = match nothing. Short-circuit with an
                // unsatisfiable predicate so the caller's
                // "no sessions found" state renders as zero rows.
                clauses.push("1 = 0".into());
            } else {
                let placeholders = vec!["?"; ids.len()].join(",");
                clauses.push(format!("e.session_id IN ({placeholders})"));
                for id in ids {
                    params.push(Box::new(id.0));
                }
            }
        }
        if let Some(correct) = filter.correct {
            clauses.push("e.correct = ?".into());
            params.push(Box::new(correct as i64));
        }
        if let Some(from) = filter.from_ms {
            clauses.push("e.ts_ms >= ?".into());
            params.push(Box::new(from));
        }
        if let Some(until) = filter.until_ms {
            clauses.push("e.ts_ms <= ?".into());
            params.push(Box::new(until));
        }
        // Per-session predicates join sessions.
        let needs_join = filter.layout_hash.is_some()
            || filter.keyboard_hash.is_some()
            || filter.exercise_category.is_some()
            || filter.exercise_categories.is_some();
        if let Some(h) = &filter.layout_hash {
            clauses.push("s.layout_hash = ?".into());
            params.push(Box::new(h.0.clone()));
        }
        if let Some(h) = &filter.keyboard_hash {
            clauses.push("s.keyboard_hash = ?".into());
            params.push(Box::new(h.0.clone()));
        }
        if let Some(cat) = &filter.exercise_category {
            clauses.push("s.exercise_category = ?".into());
            params.push(Box::new(cat.clone()));
        }
        if let Some(cats) = &filter.exercise_categories {
            if cats.is_empty() {
                clauses.push("0".into());
            } else {
                let placeholders = vec!["?"; cats.len()].join(",");
                clauses.push(format!("s.exercise_category IN ({placeholders})"));
                for c in cats {
                    params.push(Box::new(c.clone()));
                }
            }
        }

        let mut sql = String::from(
            "SELECT e.session_id, e.ts_ms, e.expected, e.typed, e.correct, e.delta_ms FROM events e",
        );
        if needs_join {
            sql.push_str(" JOIN sessions s ON s.session_id = e.session_id");
        }
        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(" ORDER BY e.ts_ms ASC");

        let mut stmt = conn.prepare(&sql)?;
        let params_slice: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|b| b.as_ref()).collect();
        let rows = stmt
            .query_map(params_slice.as_slice(), row_to_event)?
            .collect::<std::result::Result<Vec<Event>, rusqlite::Error>>()?;
        Ok(Box::new(rows.into_iter().map(Ok)))
    }

    fn sessions(&self, filter: &SessionFilter) -> Result<Vec<SessionSummary>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let mut clauses: Vec<String> = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        if let Some(h) = &filter.layout_hash {
            clauses.push("layout_hash = ?".into());
            params.push(Box::new(h.0.clone()));
        }
        if let Some(n) = &filter.layout_name {
            clauses.push("layout_name = ?".into());
            params.push(Box::new(n.clone()));
        }
        if let Some(h) = &filter.keyboard_hash {
            clauses.push("keyboard_hash = ?".into());
            params.push(Box::new(h.0.clone()));
        }
        if let Some(cat) = &filter.exercise_category {
            clauses.push("exercise_category = ?".into());
            params.push(Box::new(cat.clone()));
        }
        if let Some(from) = filter.from_ms {
            clauses.push("started_at_ms >= ?".into());
            params.push(Box::new(from));
        }
        if let Some(until) = filter.until_ms {
            clauses.push("started_at_ms <= ?".into());
            params.push(Box::new(until));
        }

        let mut sql = String::from(
            "SELECT session_id, started_at_ms, ended_at_ms, \
                    layout_hash, layout_name, keyboard_hash, keyboard_name, \
                    exercise_category, exercise_instance, \
                    total_events, total_correct \
             FROM sessions",
        );
        if !clauses.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&clauses.join(" AND "));
        }
        sql.push_str(" ORDER BY started_at_ms DESC");
        if let Some(limit) = filter.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }

        let mut stmt = conn.prepare(&sql)?;
        let params_slice: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|b| b.as_ref()).collect();
        let rows = stmt
            .query_map(params_slice.as_slice(), row_to_summary_direct)?
            .collect::<std::result::Result<Vec<SessionSummary>, rusqlite::Error>>()?;
        Ok(rows)
    }

    fn layout_snapshot(&self, hash: &LayoutHash) -> Result<Option<LayoutSnapshot>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let row = conn
            .query_row(
                "SELECT hash, name, canonical_json, first_seen_ms FROM layout_snapshots \
                 WHERE hash = ?",
                params![hash.0],
                |r| {
                    Ok(LayoutSnapshot {
                        hash: LayoutHash(r.get(0)?),
                        name: r.get(1)?,
                        canonical_json: r.get(2)?,
                        first_seen_ms: r.get(3)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    fn keyboard_snapshot(&self, hash: &KeyboardHash) -> Result<Option<KeyboardSnapshot>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let row = conn
            .query_row(
                "SELECT hash, name, canonical_json, first_seen_ms FROM keyboard_snapshots \
                 WHERE hash = ?",
                params![hash.0],
                |r| {
                    Ok(KeyboardSnapshot {
                        hash: KeyboardHash(r.get(0)?),
                        name: r.get(1)?,
                        canonical_json: r.get(2)?,
                        first_seen_ms: r.get(3)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }
}

// ---- row decoders ----

fn row_to_event(r: &rusqlite::Row<'_>) -> rusqlite::Result<Event> {
    let expected_s: String = r.get(2)?;
    let typed_s: String = r.get(3)?;
    let correct_i: i64 = r.get(4)?;
    Ok(Event {
        session_id: SessionId(r.get(0)?),
        ts_ms: r.get(1)?,
        expected: expected_s.chars().next().unwrap_or('\0'),
        typed: typed_s.chars().next().unwrap_or('\0'),
        correct: correct_i != 0,
        delta_ms: r.get(5)?,
    })
}

fn row_to_summary_direct(r: &rusqlite::Row<'_>) -> rusqlite::Result<SessionSummary> {
    Ok(SessionSummary {
        session_id: SessionId(r.get(0)?),
        started_at_ms: r.get(1)?,
        ended_at_ms: r.get(2)?,
        layout_hash: LayoutHash(r.get(3)?),
        layout_name: r.get(4)?,
        keyboard_hash: KeyboardHash(r.get(5)?),
        keyboard_name: r.get(6)?,
        exercise_category: r.get(7)?,
        exercise_instance: r.get(8)?,
        total_events: r.get::<_, i64>(9)? as u64,
        total_correct: r.get::<_, i64>(10)? as u64,
    })
}

fn row_to_summary(conn: &Connection, session_id: SessionId) -> Result<SessionSummary> {
    conn.query_row(
        "SELECT session_id, started_at_ms, ended_at_ms, \
                layout_hash, layout_name, keyboard_hash, keyboard_name, \
                exercise_category, exercise_instance, \
                total_events, total_correct \
         FROM sessions WHERE session_id = ?",
        params![session_id.0],
        row_to_summary_direct,
    )
    .map_err(Into::into)
}
