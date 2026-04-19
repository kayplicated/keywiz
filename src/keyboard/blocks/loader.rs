//! JSON5 reader for the blocks schema.
//!
//! Schema:
//! ```json5
//! {
//!   name: "halcyon_elora_v2",
//!   short: "Halcyon Elora v2",
//!   description: "...",
//!   blocks: [
//!     {
//!       type: "col-stag",          // or "row-stag" or "free-form"
//!       cluster: "main",
//!       keys: [
//!         { id: "main_k1", r: -2, c: -6, x: -6.0, y: -1.25,
//!           finger: "LPinky" },
//!         ...
//!       ]
//!     },
//!     ...
//!   ]
//! }
//! ```
//!
//! Per-key defaults: `width` / `height` → 1.0, `rotation` → 0.0. The
//! block's cluster applies to every key within it; key records don't
//! carry their own cluster.

use serde::Deserialize;
use std::path::Path;

use crate::keyboard::blocks::{
    BlockKind, BlocksKeyboard, ColStagBlock, FreeFormBlock, RowStagBlock,
};
use crate::keyboard::common::{Finger, PhysicalKey};

#[derive(Debug, Deserialize)]
struct KeyRecord {
    id: String,
    r: i32,
    c: i32,
    x: f32,
    y: f32,
    #[serde(default = "default_size")]
    width: f32,
    #[serde(default = "default_size")]
    height: f32,
    #[serde(default)]
    rotation: f32,
    #[serde(deserialize_with = "deserialize_finger")]
    finger: Finger,
}

#[derive(Debug, Deserialize)]
struct BlockRecord {
    #[serde(rename = "type")]
    kind: String,
    cluster: String,
    keys: Vec<KeyRecord>,
}

#[derive(Debug, Deserialize)]
struct KeyboardRecord {
    name: String,
    short: String,
    #[serde(default)]
    description: String,
    blocks: Vec<BlockRecord>,
}

pub fn load(path: &Path) -> Result<BlocksKeyboard, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let record: KeyboardRecord =
        json5::from_str(&content).map_err(|e| format!("parsing {}: {e}", path.display()))?;

    let mut blocks = Vec::with_capacity(record.blocks.len());
    for block in record.blocks {
        let keys = block
            .keys
            .into_iter()
            .map(|k| PhysicalKey {
                id: k.id,
                r: k.r,
                c: k.c,
                x: k.x,
                y: k.y,
                width: k.width,
                height: k.height,
                rotation: k.rotation,
                cluster: block.cluster.clone(),
                finger: k.finger,
            })
            .collect();

        let block_kind = match block.kind.as_str() {
            "row-stag" => BlockKind::RowStag(RowStagBlock {
                cluster: block.cluster,
                keys,
            }),
            "col-stag" => BlockKind::ColStag(ColStagBlock {
                cluster: block.cluster,
                keys,
            }),
            "free-form" => BlockKind::FreeForm(FreeFormBlock {
                cluster: block.cluster,
                keys,
            }),
            other => {
                return Err(format!(
                    "parsing {}: unknown block type {other:?}; expected \
                     \"row-stag\", \"col-stag\", or \"free-form\"",
                    path.display()
                ));
            }
        };
        blocks.push(block_kind);
    }

    Ok(BlocksKeyboard {
        name: record.name,
        short: record.short,
        description: record.description,
        blocks,
    })
}

fn default_size() -> f32 {
    1.0
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
