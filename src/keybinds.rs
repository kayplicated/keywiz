//! Input classification — keypress → semantic action.
//!
//! One job: sort an incoming crossterm `KeyEvent` into a `Classified`
//! variant the main loop can dispatch on. No state, no side effects.
//!
//! Control keys here route to the engine at the main-loop level
//! (toggle, cycle, quit). Typed characters flow through
//! `Engine::process_input` to the active exercise. Exercise-specific
//! shortcuts (e.g. TextExercise passage switching) aren't modeled
//! here yet — add them when a second exercise needs control keys
//! that aren't engine-scoped.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub enum Classified {
    Quit,
    Typing(char),
    /// Tab — hide/show whatever's currently occupying the
    /// keyboard slot. A real toggle, not a cycle.
    ToggleSlot,
    /// Shift+Tab — toggle the flash-on-keypress layer.
    /// Orthogonal to F2 overlays: flash stacks on top of whatever
    /// overlay is painting, so you can have heat + flash together.
    ToggleFlash,
    /// F1 — toggle the help page listing every keybind.
    ToggleHelpPage,
    /// F2 — cycle overlay (none → finger → heat → none).
    ToggleHeatmap,
    /// F3 — cycle what's shown in the keyboard slot (keyboard ↔
    /// inline stats ↔ future slot contents). Orthogonal to Tab.
    CycleSlot,
    /// F4 — toggle the full stats page modal. Typing pauses;
    /// navigation keybinds apply to stats views instead of
    /// keyboard/layout cycling.
    ToggleStatsPage,
    /// F5 — toggle the layout-iterations modal. Different
    /// question from F4 (stats are about "how am I typing"; F5
    /// is about "how is the *layout* performing across its
    /// iterations"). Typing pauses; Esc or F5 closes.
    ToggleLayoutPage,
    NextKeyboard,
    PrevKeyboard,
    NextLayout,
    PrevLayout,
    /// Alt+↓ — next exercise category (drill / words / text).
    NextExerciseCategory,
    /// Alt+↑ — previous exercise category.
    PrevExerciseCategory,
    /// Alt+→ — next instance within the current category.
    NextExerciseInstance,
    /// Alt+← — previous instance within the current category.
    PrevExerciseInstance,
    /// Any key that the main loop has no binding for — arrows
    /// without modifiers, function keys we don't handle, etc.
    /// Swallowed silently.
    Ignored,
}

pub fn classify(key: KeyEvent) -> Classified {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    match key.code {
        KeyCode::Esc => Classified::Quit,
        KeyCode::Tab => Classified::ToggleSlot,
        KeyCode::BackTab => Classified::ToggleFlash,
        KeyCode::F(1) => Classified::ToggleHelpPage,
        KeyCode::F(2) => Classified::ToggleHeatmap,
        KeyCode::F(3) => Classified::CycleSlot,
        KeyCode::F(4) => Classified::ToggleStatsPage,
        KeyCode::F(5) => Classified::ToggleLayoutPage,
        KeyCode::Up if ctrl => Classified::PrevKeyboard,
        KeyCode::Down if ctrl => Classified::NextKeyboard,
        KeyCode::Left if ctrl => Classified::PrevLayout,
        KeyCode::Right if ctrl => Classified::NextLayout,
        KeyCode::Up if alt => Classified::PrevExerciseCategory,
        KeyCode::Down if alt => Classified::NextExerciseCategory,
        KeyCode::Left if alt => Classified::PrevExerciseInstance,
        KeyCode::Right if alt => Classified::NextExerciseInstance,
        KeyCode::Char(ch) => Classified::Typing(ch),
        _ => Classified::Ignored,
    }
}
