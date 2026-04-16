//! Centralized configuration constants and defaults.

/// Rolling window size for drill accuracy tracking.
pub const WINDOW_SIZE: usize = 20;

/// Accuracy threshold (%) to level up in drills.
pub const LEVEL_UP_THRESHOLD: f64 = 90.0;

/// Accuracy threshold (%) to level down in drills.
pub const LEVEL_DOWN_THRESHOLD: f64 = 70.0;

/// Minimum keys typed at a level before a level change can occur.
pub const MIN_KEYS_BEFORE_LEVEL_CHANGE: usize = 30;
