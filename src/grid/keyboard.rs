//! Load physical keyboard definitions from JSON.
//!
//! Schema:
//! ```json
//! {
//!   "name": "us_intl",
//!   "short": "US International",
//!   "description": "optional longer text",
//!   "buttons": [
//!     { "code": "KEY_A", "x": -5.0, "y": 0.0, "finger": "LPinky" },
//!     ...
//!   ]
//! }
//! ```

use serde::Deserialize;
use std::path::Path;

use crate::grid::Finger;

/// One physical key on a keyboard. Coordinates are home-row-centered: x
/// grows right, y grows down, one unit = one key width / row height.
#[derive(Debug, Clone, Deserialize)]
pub struct KeyboardButton {
    pub code: String,
    pub x: f32,
    pub y: f32,
    #[serde(deserialize_with = "deserialize_finger")]
    pub finger: Finger,
}

/// A physical keyboard's full button set, as loaded from disk.
#[derive(Debug, Clone, Deserialize)]
pub struct Keyboard {
    pub name: String,
    pub short: String,
    pub buttons: Vec<KeyboardButton>,
}

impl Keyboard {
    /// Load a keyboard from a JSON file. Parse errors are surfaced so the
    /// user can fix a broken file instead of silently getting an empty
    /// keyboard.
    pub fn load(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("parsing {}: {e}", path.display()))
    }
}

/// Deserialize a [`Finger`] from its enum name (`"LPinky"`, `"RIndex"`, …).
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
        "RIndex" => Ok(Finger::RIndex),
        "RMiddle" => Ok(Finger::RMiddle),
        "RRing" => Ok(Finger::RRing),
        "RPinky" => Ok(Finger::RPinky),
        other => Err(serde::de::Error::custom(format!(
            "unknown finger {other:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_keyboard() {
        let json = r#"{
            "name": "test",
            "short": "Test",
            "buttons": [
                { "code": "KEY_A", "x": -5.0, "y": 0.0, "finger": "LPinky" },
                { "code": "KEY_J", "x":  1.0, "y": 0.0, "finger": "RIndex" }
            ]
        }"#;
        let kb: Keyboard = serde_json::from_str(json).unwrap();
        assert_eq!(kb.name, "test");
        assert_eq!(kb.buttons.len(), 2);
        assert_eq!(kb.buttons[0].code, "KEY_A");
        assert!(matches!(kb.buttons[1].finger, Finger::RIndex));
    }

}
