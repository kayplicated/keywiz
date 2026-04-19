//! Shared application context passed to all modes.

use crate::engine::{Engine, Translator};
use crate::keyboard::Keyboard;
use crate::mapping::{KeyMapping, Layout};
use crate::stats::StatsTracker;
use crate::typing::drill::{CharSource, DrillLevel};

pub struct AppContext {
    pub(crate) show_keyboard: bool,
    pub(crate) show_heatmap: bool,
    pub(crate) translator: Translator,
    pub(crate) from_layout: Option<String>,
    pub(crate) stats: StatsTracker,
    pub(crate) engine: Engine,
}

impl AppContext {
    pub fn new(engine: Engine, from_layout: Option<String>) -> Self {
        let translator = Translator::identity();
        let mut ctx = Self {
            show_keyboard: true,
            show_heatmap: false,
            translator,
            from_layout: from_layout.clone(),
            stats: StatsTracker::new(),
            engine,
        };
        ctx.rebuild_translator();
        ctx
    }

    pub fn translate_input(&self, ch: char) -> char {
        self.translator.translate(ch)
    }

    pub fn keyboard(&self) -> &dyn Keyboard {
        self.engine.keyboard()
    }

    pub fn layout(&self) -> &Layout {
        self.engine.layout()
    }

    /// Rebuild the input-character translator against the current
    /// layout. Called after keyboard/layout changes so `--from`
    /// keeps tracking the active target.
    pub fn rebuild_translator(&mut self) {
        self.translator =
            crate::engine::translate::build(self.engine.layout(), self.from_layout.as_deref());
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn engine_mut(&mut self) -> &mut Engine {
        &mut self.engine
    }

    pub fn stats_key(&self) -> &str {
        self.engine.current_layout()
    }
}

impl CharSource for AppContext {
    fn chars_for(&self, level: DrillLevel) -> Vec<char> {
        let want_rows: &[i32] = match level {
            DrillLevel::HomeRow => &[0],
            DrillLevel::HomeAndTop => &[0, -1],
            DrillLevel::AllRows => &[-2, -1, 0, 1],
        };
        let keyboard = self.keyboard();
        let layout = self.layout();
        keyboard
            .keys()
            .filter(|k| want_rows.contains(&k.r))
            .filter_map(|k| match layout.get(&k.id) {
                Some(KeyMapping::Char { lower, .. }) if lower.is_alphabetic() => Some(*lower),
                _ => None,
            })
            .collect()
    }
}
