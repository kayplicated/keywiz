//! Clusters — groupings of keys that share a logical region.
//!
//! A cluster is just a name. Keyboards assign one to each key via the
//! [`crate::physical::keys::PhysicalKey::cluster`] field; renderers and
//! layouts use it to group keys visually or functionally.
//!
//! # Suggested convention (not enforced)
//!
//! - `"main"` — the alpha block (letters + numbers + punctuation drills)
//! - `"mods"` — modifier/system keys (Esc, Tab, Shift, Ctrl, Alt, …)
//! - `"fn"` — function-key row (F1..F24)
//! - `"nav"` — navigation island (arrows, home/end, pageup/down, …)
//! - `"num_pad"` — numpad block
//! - `"left_thumb"`, `"right_thumb"` — thumb clusters on split/ergo boards
//! - `"left_macro"`, `"right_macro"` — optional macro columns
//!
//! Clusters drive two things: the id convention (some clusters use
//! `{cluster}_k{n}` indexed ids, others use `{cluster}_{semantic_name}`
//! — see [`super::naming`] when implemented), and renderer decisions
//! (terminal may draw thumb clusters as a separate block).

/// Cluster name — free-form string so new clusters are always possible.
///
/// Uses `String` rather than an enum because the naming convention is a
/// social contract, not something the compiler should police.
pub type Cluster = String;

/// Default cluster name for keys that don't specify one.
pub const DEFAULT_CLUSTER: &str = "main";
