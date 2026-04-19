mod app;
mod config;
mod configreader;
mod engine;
mod grid;
mod keybinds;
mod mode;
mod prefs;
mod stats;
mod translate;
mod ui;
mod words;

use std::io;

use app::AppContext;
use configreader::kanata::KanataReader;
use configreader::ConfigReader;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use keybinds::KeybindResult;
use mode::{ActiveMode, ModeResult};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use translate::Translator;

fn main() -> io::Result<()> {
    // Parse args
    let args: Vec<String> = std::env::args().collect();
    let from_layout = flag_value(&args, "--from");
    let keyboard_flag = flag_value(&args, "-k").or_else(|| flag_value(&args, "--keyboard"));
    let layout_flag = flag_value(&args, "-l").or_else(|| flag_value(&args, "--layout"));
    let kanata_path = flag_value(&args, "--kanata");

    // Collect indices of args consumed by value-taking flags so positional
    // collection skips both the flag and its value.
    let mut skip_indices = std::collections::HashSet::new();
    for (i, a) in args.iter().enumerate() {
        let a = a.as_str();
        if matches!(
            a,
            "--from" | "-k" | "--keyboard" | "-l" | "--layout" | "--kanata"
        ) {
            skip_indices.insert(i);
            skip_indices.insert(i + 1);
        }
    }
    let positional: Vec<&String> = args
        .iter()
        .enumerate()
        .filter(|(i, _)| *i > 0 && !skip_indices.contains(i))
        .map(|(_, a)| a)
        .collect();

    // Branch: --kanata loads from a .kbd; otherwise data-driven grid path.
    // Kanata is always invoked explicitly so prefs don't apply there.
    let is_kanata = kanata_path.is_some();
    let mut ctx = if let Some(path) = kanata_path {
        build_kanata_context(&path, positional.first().map(|s| s.as_str()), from_layout.as_deref())?
    } else {
        // Load last-used prefs as fallback defaults; explicit flags override.
        let saved = prefs::Prefs::load();
        let keyboard = keyboard_flag.as_deref().or(saved.keyboard.as_deref());
        let layout = layout_flag.as_deref().or(saved.layout.as_deref());
        build_grid_context(keyboard, layout, from_layout.as_deref())?
    };

    // Setup terminal
    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    ctx.stats
        .set_persistent(stats::persist::load(ctx.stats_key()));

    let result = run_loop(&mut terminal, &mut ctx);

    // Restore terminal — ignore errors (can fail over SSH)
    let _ = terminal::disable_raw_mode();
    let _ = io::stdout().execute(LeaveAlternateScreen);

    stats::persist::save(ctx.stats_key(), ctx.stats.persistent());
    if !is_kanata {
        prefs::Prefs::save(
            ctx.grid_manager.current_keyboard(),
            ctx.stats_key(),
        );
    }

    result
}

/// Parse `--flag <value>` style arguments. Returns the value if present.
fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string())
}

/// Build an [`AppContext`] via the data-driven grid path.
fn build_grid_context(
    keyboard: Option<&str>,
    layout: Option<&str>,
    from_layout: Option<&str>,
) -> io::Result<AppContext> {
    let mut manager = grid::GridManager::new().unwrap_or_else(|e| {
        eprintln!("keywiz: could not load keyboards/layouts: {e}");
        std::process::exit(1);
    });
    if let Some(name) = keyboard
        && let Err(e) = manager.set_keyboard(name)
    {
        eprintln!("keywiz: {e}");
        std::process::exit(1);
    }
    if let Some(name) = layout
        && let Err(e) = manager.set_layout(name)
    {
        eprintln!("keywiz: {e}");
        std::process::exit(1);
    }

    let translator = build_translator(manager.grid().clone(), from_layout);
    Ok(AppContext::new(manager, translator))
}

/// Build an [`AppContext`] from a kanata `.kbd` config via the kanata
/// reader. The reader produces a [`Grid`] that flows through the same
/// path as JSON-loaded grids.
fn build_kanata_context(
    config_path: &str,
    layer_selector: Option<&str>,
    from_layout: Option<&str>,
) -> io::Result<AppContext> {
    let source = std::fs::read_to_string(config_path).unwrap_or_else(|e| {
        eprintln!("keywiz: could not read {config_path}: {e}");
        std::process::exit(1);
    });

    let reader = KanataReader;
    let grid = reader.read(&source, layer_selector).unwrap_or_else(|e| {
        eprintln!("keywiz: {} reader: {e}", reader.format_name());
        std::process::exit(1);
    });

    let translator = build_translator(grid.clone(), from_layout);
    let manager = grid::GridManager::single(grid);
    Ok(AppContext::new(manager, translator))
}

/// Build a translator from the active [`Grid`] back to the input keyboard.
/// `from_layout` names a layout in `layouts/` (e.g. `"qwerty"`) describing
/// what the user's physical keyboard actually sends.
fn build_translator(target: grid::Grid, from_layout: Option<&str>) -> Translator {
    let Some(from_name) = from_layout else {
        return Translator::identity();
    };
    // Compose the from-layout against the same keyboard so positional
    // semantics match.
    let from_path = std::path::Path::new("layouts").join(format!("{from_name}.json"));
    let from_layout_data = grid::Layout::load(&from_path).unwrap_or_else(|e| {
        eprintln!("keywiz: --from {from_name}: {e}");
        std::process::exit(1);
    });
    let kb_path = std::path::Path::new("keyboards").join(format!("{}.json", target.keyboard_name));
    let keyboard = grid::Keyboard::load(&kb_path).unwrap_or_else(|_| {
        // Kanata-derived grids don't have a matching keyboard file — fall
        // back to us_intl so translation still works.
        grid::Keyboard::load(std::path::Path::new("keyboards/us_intl.json"))
            .expect("us_intl.json should always be present")
    });
    let from_grid = grid::Grid::compose(&keyboard, &from_layout_data);
    Translator::between(&from_grid, &target)
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ctx: &mut AppContext,
) -> io::Result<()> {
    let mut mode = ActiveMode::Select(mode::select::SelectMode);

    loop {
        terminal.draw(|f| mode.render(f, ctx))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // Global keybinds — handled once, before per-mode dispatch.
            if matches!(keybinds::handle_shared(key, ctx), KeybindResult::Handled) {
                continue;
            }

            // Mode-specific handling
            match mode.handle_input(key, ctx) {
                ModeResult::Stay => {}
                ModeResult::Quit => return Ok(()),
                ModeResult::SwitchTo(new_mode) => {
                    // Save when returning to menu (session boundary).
                    if matches!(new_mode, ActiveMode::Select(_)) {
                        stats::persist::save(ctx.stats_key(), ctx.stats.persistent());
                    }
                    mode = new_mode;
                }
            }
        }
    }
}
