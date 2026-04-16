# CLAUDE.md — Rules for Working on Keywiz

**For:** Claude instances working with Kay on keywiz
**What:** Terminal typing tutor with visual keyboard for custom layouts.
Reads kanata config files, renders a color-coded keyboard, and offers
multiple training modes. Single binary, single crate, runs in any
terminal.

---

## Read Before Coding

Read the code. Understand the module you're changing and its
neighbors before editing anything. Don't guess at structure — look.

The code is the source of truth. There are no separate spec files.

---

## Architecture

### Three layers: modes, engines, UI components

Keywiz has three kinds of code. Every file belongs to exactly one:

**Modes** — the training experiences. A mode provides content (what
to type), flow (when it's done, what comes next), and wires together
engines and UI components. Modes live in `mode/`. Adding a mode
should not require changes to engines or UI components — only new
wiring.

**Engines** — the reusable mechanics. The typing engine handles
char-by-char matching, accuracy, and WPM. The drill engine handles
single-char presentation and level progression. Engines live in
`engine/`. An engine knows nothing about which mode is using it.

**UI components** — reusable rendering pieces. The keyboard widget,
the scrolling text display, the layout skeleton. Live in `ui/`. A
component renders what it's told — it doesn't know about modes or
game state.

**The rule:** modes compose engines and components. Engines don't
know about modes. Components don't know about engines or modes.
Dependencies flow one way: mode → engine, mode → component. Never
the reverse.

### Module map

```
src/
  main.rs              — arg parsing, terminal setup, event loop
  app.rs               — AppContext (shared state, config, layout)
  config.rs            — thresholds, defaults, settings
  mode/
    mod.rs             — Mode trait, ModeResult, shared key handling
    select.rs          — mode selection menu
    drill.rs           — key drill (single char, adaptive levels)
    words.rs           — random word typing
    text.rs            — passage typing
  engine/
    typing.rs          — char matching, WPM, accuracy
    drill.rs           — char presentation, level progression
  layout/
    mod.rs             — Layout, Key, Row, Finger types
    kanata.rs          — kanata config parser
  stats.rs             — per-key stats, session tracking
  ui/
    mod.rs             — shared layout skeleton, centered_layout
    keyboard.rs        — keyboard widget
    text_display.rs    — scrolling styled text with cursor
  words.rs             — word list loading
```

This is the target structure. The codebase is being refactored
toward it. When adding new code, put it where this map says it
goes — don't add to the old locations.

### AppContext

`AppContext` holds everything shared across modes: layout, config,
stats, display toggles (split, show_keyboard), input translation.
Modes receive `&mut AppContext` — they don't own shared state.

### The Mode trait

```rust
trait Mode {
    fn handle_input(&mut self, key: KeyEvent, ctx: &mut AppContext) -> ModeResult;
    fn render(&self, f: &mut Frame, ctx: &AppContext);
}
```

`ModeResult` tells the event loop what to do: stay, quit, or
switch to another mode. The event loop handles shared keys (Esc,
Tab, BackTab) before dispatching to the active mode.

### Adding a new mode

1. Create `mode/yourmode.rs`
2. Implement `Mode` — wire in whichever engines and UI components
   you need
3. Add it to the mode selection menu
4. Done. No changes to the event loop, no changes to other modes.

If adding a mode requires changing the event loop or another mode,
the architecture is broken — fix the architecture, don't hack
around it.

---

## Layout

### Parser boundary

Nothing outside `layout/` knows about kanata internals. `layout/`
exposes `Layout`, `Key`, `Row`, `Finger`, and a public parsing
API. If a new layout format is added, it becomes another file in
`layout/` producing the same `Layout` type.

### Finger assignment

`Layout` handles finger-to-key mapping for both row-stagger and
column-stagger keyboards. The `set_colstag()` toggle reassigns
fingers. Modes and UI don't compute finger assignments — they
read them from the layout.

---

## State Ownership

### Mode state belongs to the mode

A drill mode owns its drill engine state. A typing mode owns its
typing engine state. State is not stored as `Option<Thing>` outside
the mode.

### Shared state lives in AppContext

Layout, config, stats, display toggles — these cross mode
boundaries and live in `AppContext`. Modes borrow it, they don't
duplicate it.

### No pub fields on types that cross boundaries

Types passed between layers (engines ↔ modes, modes ↔ UI) use
methods, not public fields. Internal state stays internal.

---

## Code Rules

### File scope

- Every file is one module, one job
- If a file exceeds ~200 lines, consider splitting it
- If a function exceeds ~40 lines, it's doing too much

### Don't duplicate

- Key resolution: one function, not two
- Shifted-char mapping: one place
- UI layout skeleton: one helper, used by all modes
- Shared input handling (Tab, Esc, BackTab): one place in the
  event loop

### No feature removal

Do not remove existing features to make room for new ones. If a
feature is in the way, the structure is wrong — restructure. If
restructuring is too big for the current task, stop and discuss.

### Dependencies

Keep the dependency footprint small. Current deps: `ratatui`,
`crossterm`, `rand`. Add a crate when the alternative is writing
something error-prone by hand (arg parsing, file format parsing).
Don't add crates for trivial tasks.

---

## Documentation

### Rustdoc comments

`///` on every public type, method, and function. `//!` at the top
of every module file explaining what the module is and what it does.

### Comment conventions

- `// TODO:` — tracked work, include context on what needs doing
- `// HACK:` — known shortcut, explain why and what the real fix is

### No separate docs

No markdown files except this CLAUDE.md and README.md. The code
documents itself through rustdoc comments.

---

## Tests

Tests live in `#[cfg(test)] mod tests` within the module they test.
Focus on engines and layout parsing — these have clear inputs and
outputs. Modes and UI components are tested manually in the terminal.

Don't write tests that duplicate what the engine does. Test behavior:
"given these inputs, this output." Not mechanics: "this field is set
to this value."

---

## Git

- `master` is stable
- Small, frequent commits that compile on their own
- Prefix commits with the area: `mode/drill:`, `engine/typing:`,
  `layout:`, `ui:`, etc.

---

## Building and Running

```bash
cargo run                                    # default layout
cargo run -- layouts/gallium_v2.kbd          # specific layout
cargo run -- --split layouts/gallium_v2.kbd  # split keyboard
cargo run -- --from qwerty layouts/gallium_v2.kbd  # input translation
cargo test                                   # run tests
cargo clippy                                 # lint
```
