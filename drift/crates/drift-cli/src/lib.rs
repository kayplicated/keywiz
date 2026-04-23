//! Library surface of `drift-cli`.
//!
//! The `drift` binary in `src/main.rs` is a thin wrapper over
//! [`dispatch`]. External consumers (e.g. keywiz's `--drift`
//! passthrough) call [`dispatch_args`] to forward their own argv
//! into drift's CLI without shelling out.

pub mod commands;

pub use commands::{dispatch, dispatch_args};
