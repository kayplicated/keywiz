//! Load character-mapping layouts from JSON.
//!
//! Schema:
//! ```json
//! {
//!   "name": "qwerty",
//!   "short": "QWERTY",
//!   "keys": {
//!     "KEY_A": { "lower": "a", "upper": "A" },
//!     ...
//!   }
//! }
//! ```

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// What a single keycode produces — the unshifted character plus its shift
/// variant (the character that results when shift is held).
#[derive(Debug, Clone, Deserialize)]
pub struct KeyMapping {
    pub lower: char,
    pub upper: char,
}

/// A character-mapping layout as loaded from disk.
#[derive(Debug, Clone, Deserialize)]
pub struct Layout {
    pub name: String,
    pub short: String,
    pub keys: HashMap<String, KeyMapping>,
}

impl Layout {
    pub fn load(path: &Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("parsing {}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_layout() {
        let json = r#"{
            "name": "test",
            "short": "T",
            "keys": {
                "KEY_A": { "lower": "a", "upper": "A" },
                "KEY_SEMICOLON": { "lower": ";", "upper": ":" }
            }
        }"#;
        let lo: Layout = serde_json::from_str(json).unwrap();
        assert_eq!(lo.keys.len(), 2);
        assert_eq!(lo.keys["KEY_A"].lower, 'a');
        assert_eq!(lo.keys["KEY_SEMICOLON"].upper, ':');
    }
}
