//! Third-party tool integrations.
//!
//! Keywiz wraps external software — kanata configs for import,
//! oxeylyzer for analysis (later), qmk/zmk firmware formats (later).
//! Each lives as a sibling module here.
//!
//! This is *not* a plugin system. Nothing loads into keywiz at
//! runtime; these are keywiz calling out to other tools or reading
//! their file formats.

pub mod kanata;
