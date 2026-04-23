//! JSON view of a layout diff.
//!
//! Returns a borrowed `Serialize` payload rather than a
//! pre-rendered string so the CLI can nest the diff inside its
//! larger `compare --format json` envelope without re-parsing.

use serde::Serialize;

use super::compute::DiffEntry;

#[derive(Serialize)]
pub struct DiffPayload<'a> {
    pub a_name: &'a str,
    pub b_name: &'a str,
    pub entries: &'a [DiffEntry],
}

/// Bundle diff entries with the labels that describe them.
pub fn payload<'a>(
    entries: &'a [DiffEntry],
    a_name: &'a str,
    b_name: &'a str,
) -> DiffPayload<'a> {
    DiffPayload { a_name, b_name, entries }
}
