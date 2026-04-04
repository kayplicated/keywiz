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
}

impl App {
    pub fn new(layout: Layout) -> Self {
        Self {
            layout,
            mode: Mode::ModeSelect,
            should_quit: false,
        }
    }
}
