//! Concrete trigram rules.
//!
//! Each submodule implements one or more [`super::TrigramRule`].
//! To add a new rule, drop a new file here and wire it up in
//! [`super::registry::construct_rule`].

pub mod alternate;
pub mod flexion_cascade;
pub mod hand_territory;
pub mod onehand;
pub mod pinky_terminal;
pub mod redirect;
pub mod roll;
pub mod row_cascade;
