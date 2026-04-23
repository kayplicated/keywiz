//! Preset resolution — named bundles of config defaults.
//!
//! Shipped presets live as TOML files under the crate's `presets/`
//! directory. They're embedded via `include_str!` so the binary
//! carries them at build time rather than expecting them on disk.
//!
//! - `neutral`: all opinion-bearing analyzers disabled or weight 0.
//! - `drifter`: flexion-favoring, hand-territory-aware defaults.

use std::path::PathBuf;

use anyhow::{anyhow, Result};

/// Return the on-disk path for a named preset.
///
/// Currently resolves relative to the drift-config crate root. Once
/// presets move to embedded bundles, this will be replaced with an
/// in-memory load path.
pub fn path_for(name: &str) -> Result<PathBuf> {
    let crate_root = env!("CARGO_MANIFEST_DIR");
    let path = PathBuf::from(crate_root).join("presets").join(format!("{name}.toml"));
    if !path.exists() {
        return Err(anyhow!("unknown preset: {name:?} (expected {})", path.display()));
    }
    Ok(path)
}
