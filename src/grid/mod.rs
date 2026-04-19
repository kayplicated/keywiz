//! Composition of a physical keyboard with a character layout.
//!
//! [`crate::physical`] owns the hardware model — where keys sit, which
//! finger reaches them. [`Layout`] owns the character mapping — which
//! id each physical position is bound to. A [`Grid`] binds the two:
//! one [`GridButton`] per physical key, with the mapping resolved in
//! (or `None` if the layout doesn't cover that id).
//!
//! The layout's domain is ids. Keys not in the layout's domain render
//! as dead outlines — not an error, just "this switch isn't used under
//! this layout."

pub mod layout;
pub mod manager;

pub use layout::{KeyMapping, Layout};
pub use manager::GridManager;

use crate::physical::engine::{Cluster, Finger};
use crate::physical::keys::PhysicalKey;
use crate::physical::PhysicalKeyboard;

/// A physical key composed with its (optional) character mapping.
#[derive(Debug, Clone)]
pub struct GridButton {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
    pub cluster: Cluster,
    pub finger: Finger,
    /// `None` when the active layout doesn't cover this id. The widget
    /// still draws the button as an empty, dimmed outline.
    pub mapping: Option<KeyMapping>,
}

impl GridButton {
    fn from_physical(key: &PhysicalKey, mapping: Option<KeyMapping>) -> Self {
        GridButton {
            id: key.id.clone(),
            x: key.x,
            y: key.y,
            width: key.width,
            height: key.height,
            rotation: key.rotation,
            cluster: key.cluster.clone(),
            finger: key.finger,
            mapping,
        }
    }
}

/// The active keyboard + layout as a flat list of drawable buttons.
#[derive(Debug, Clone)]
pub struct Grid {
    pub keyboard_name: String,
    pub keyboard_short: String,
    pub layout_name: String,
    pub layout_short: String,
    pub buttons: Vec<GridButton>,
}

impl Grid {
    /// Compose a physical keyboard with a character layout.
    pub fn compose(keyboard: &PhysicalKeyboard, layout: &Layout) -> Self {
        let buttons = keyboard
            .keys
            .iter()
            .map(|k| GridButton::from_physical(k, layout.get(&k.id).cloned()))
            .collect();
        Grid {
            keyboard_name: keyboard.name.clone(),
            keyboard_short: keyboard.short.clone(),
            layout_name: layout.name.clone(),
            layout_short: layout.short.clone(),
            buttons,
        }
    }

    /// All alphabetic characters produced by buttons on the row nearest
    /// `y`. Snaps each button's fractional y to its nearest integer row
    /// so column-stagger splay doesn't split keys across adjacent rows.
    pub fn alpha_chars_at_row(&self, row: i32) -> Vec<char> {
        self.buttons
            .iter()
            .filter(|b| b.y.round() as i32 == row)
            .filter_map(|b| match &b.mapping {
                Some(KeyMapping::Char { lower, .. }) => Some(*lower),
                _ => None,
            })
            .filter(|c| c.is_alphabetic())
            .collect()
    }

    pub fn home_row_chars(&self) -> Vec<char> {
        self.alpha_chars_at_row(0)
    }

    pub fn home_and_top_chars(&self) -> Vec<char> {
        let mut chars = self.alpha_chars_at_row(0);
        chars.extend(self.alpha_chars_at_row(-1));
        chars
    }

    pub fn all_alpha_chars(&self) -> Vec<char> {
        self.buttons
            .iter()
            .filter_map(|b| match &b.mapping {
                Some(KeyMapping::Char { lower, .. }) => Some(*lower),
                _ => None,
            })
            .filter(|c| c.is_alphabetic())
            .collect()
    }
}
