//! JSON reader for physical keyboards.
//!
//! Schema:
//! ```json
//! {
//!   "name": "halcyon_elora_v2",
//!   "short": "Halcyon Elora v2",
//!   "description": "optional longer text",
//!   "keys": [
//!     {
//!       "id": "main_k1",
//!       "x": -5.0, "y": -1.0,
//!       "width": 1.0, "height": 1.0,
//!       "rotation": 0.0,
//!       "cluster": "main",
//!       "finger": "LPinky"
//!     },
//!     ...
//!   ]
//! }
//! ```
//!
//! Defaults: `width`/`height` → 1.0, `rotation` → 0.0, `cluster` → "main",
//! `description` → "". Unknown fingers surface as parse errors.

use serde::Deserialize;
use std::path::Path;

use crate::physical::engine::{Cluster, Finger, DEFAULT_CLUSTER};
use crate::physical::keys::{PhysicalKey, PhysicalKeyboard};

/// On-disk key record. Kept separate from [`PhysicalKey`] so the JSON
/// surface and the runtime type can evolve independently.
#[derive(Debug, Deserialize)]
struct KeyRecord {
    id: String,
    x: f32,
    y: f32,
    #[serde(default = "default_size")]
    width: f32,
    #[serde(default = "default_size")]
    height: f32,
    #[serde(default)]
    rotation: f32,
    #[serde(default = "default_cluster")]
    cluster: String,
    #[serde(deserialize_with = "deserialize_finger")]
    finger: Finger,
}

/// On-disk keyboard record.
#[derive(Debug, Deserialize)]
struct KeyboardRecord {
    name: String,
    short: String,
    #[serde(default)]
    description: String,
    keys: Vec<KeyRecord>,
}

/// Load a keyboard JSON file.
pub fn load(path: &Path) -> Result<PhysicalKeyboard, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let record: KeyboardRecord =
        serde_json::from_str(&content).map_err(|e| format!("parsing {}: {e}", path.display()))?;

    let keys = record
        .keys
        .into_iter()
        .map(|r| PhysicalKey {
            id: r.id,
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
            rotation: r.rotation,
            cluster: r.cluster as Cluster,
            finger: r.finger,
        })
        .collect();

    Ok(PhysicalKeyboard {
        name: record.name,
        short: record.short,
        description: record.description,
        keys,
    })
}

fn default_size() -> f32 {
    1.0
}

fn default_cluster() -> String {
    DEFAULT_CLUSTER.to_string()
}

fn deserialize_finger<'de, D>(deserializer: D) -> Result<Finger, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    match s.as_str() {
        "LPinky" => Ok(Finger::LPinky),
        "LRing" => Ok(Finger::LRing),
        "LMiddle" => Ok(Finger::LMiddle),
        "LIndex" => Ok(Finger::LIndex),
        "LThumb" => Ok(Finger::LThumb),
        "RThumb" => Ok(Finger::RThumb),
        "RIndex" => Ok(Finger::RIndex),
        "RMiddle" => Ok(Finger::RMiddle),
        "RRing" => Ok(Finger::RRing),
        "RPinky" => Ok(Finger::RPinky),
        other => Err(serde::de::Error::custom(format!(
            "unknown finger {other:?}"
        ))),
    }
}
