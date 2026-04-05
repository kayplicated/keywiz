use std::collections::HashMap;

use crate::layout::Layout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    ModeSelect,
    Drill,
    Typing,
}

pub struct App {
    pub layout: Layout,
    pub mode: Mode,
    pub should_quit: bool,
    pub split: bool,
    pub show_keyboard: bool,
    /// Input translation map: physical key → target layout key.
    /// None means input is already in the target layout.
    pub translate: Option<HashMap<char, char>>,
}

impl App {
    pub fn new(layout: Layout, split: bool, translate: Option<HashMap<char, char>>) -> Self {
        Self {
            layout,
            mode: Mode::ModeSelect,
            should_quit: false,
            split,
            show_keyboard: true,
            translate,
        }
    }

    /// Translate input character if translation is active.
    pub fn translate_input(&self, ch: char) -> char {
        match &self.translate {
            Some(map) => map.get(&ch).copied().unwrap_or(ch),
            None => ch,
        }
    }
}
