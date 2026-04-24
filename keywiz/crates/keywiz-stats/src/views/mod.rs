//! Derived statistics — free functions over an [`EventStore`].
//!
//! Each submodule owns one stat. No shared trait, no registry: each
//! view has the signature that makes sense for its use case (some
//! return scalars, some return maps, some return sorted vecs).
//! Uniformity for its own sake would cost more than it buys — a
//! `trait View` can land later if a "run every view" need appears.

pub mod bigram;
pub mod heat;
pub mod keys;
pub mod progression;
pub mod rhythm;
pub mod session_live;
pub mod usage;
pub mod wpm;
