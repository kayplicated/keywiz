//! Stock drift analyzers.
//!
//! Each submodule implements one analyzer type. Call
//! [`register_all`] at program start to add every stock analyzer
//! to a [`Registry`](drift_analyzer::Registry). Third-party
//! analyzers can live in separate crates and register the same way.
//!
//! Bias-audit companion: several of these analyzers implement
//! opinion-bearing rules. Opinion lives in config defaults shipped
//! via drift-config presets (neutral.toml, drifter.toml), not in
//! the analyzer code itself.

use drift_analyzer::Registry;

mod finger_util;
mod row_util;
mod trigram_util;

pub mod alternate;
pub mod async_hand_drift;
pub mod extension_cascade;
pub mod finger_load;
pub mod flexion_cascade;
pub mod hand_territory;
pub mod inward_roll;
pub mod onehand;
pub mod outward_roll;
pub mod redirect;
pub mod roll;
pub mod row_cascade;
pub mod row_distribution;
pub mod same_row_skip;
pub mod same_row_skip_fingerpair;
pub mod scissor;
pub mod sfb;
pub mod sfs;
pub mod stretch;
pub mod terminal_penalty;

/// Register every stock analyzer with the given registry.
pub fn register_all(registry: &mut Registry) {
    alternate::register(registry);
    async_hand_drift::register(registry);
    extension_cascade::register(registry);
    finger_load::register(registry);
    flexion_cascade::register(registry);
    hand_territory::register(registry);
    inward_roll::register(registry);
    onehand::register(registry);
    outward_roll::register(registry);
    redirect::register(registry);
    roll::register(registry);
    row_cascade::register(registry);
    row_distribution::register(registry);
    same_row_skip::register(registry);
    same_row_skip_fingerpair::register(registry);
    scissor::register(registry);
    sfb::register(registry);
    sfs::register(registry);
    stretch::register(registry);
    terminal_penalty::register(registry);
}
