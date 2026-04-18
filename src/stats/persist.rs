//! Disk persistence for per-layout [`Stats`](super::Stats).
//!
//! Loads from and saves to the OS-appropriate data directory:
//! - Linux: `~/.local/share/keywiz/stats/<layout>.json`
//! - macOS: `~/Library/Application Support/keywiz/stats/<layout>.json`
//! - Windows: `%APPDATA%\keywiz\stats\<layout>.json`
//!
//! TODO: wire this up in step 2 — serde + dirs crates, load on startup,
//! save on quit.
