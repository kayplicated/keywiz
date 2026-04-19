//! Keys that plug into the [`crate::physical::engine`] coordinate system.

pub mod key;
pub mod keyboard;
pub mod naming;

pub use key::PhysicalKey;
pub use keyboard::PhysicalKeyboard;
pub use naming::human_name;
