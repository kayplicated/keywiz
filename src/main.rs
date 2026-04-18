mod app;
mod config;
mod engine;
mod grid;
mod keybinds;
mod layout;
mod mode;
mod stats;
mod ui;
mod words;

use std::io;

use app::AppContext;
use crossterm::event::{self, Event, KeyEventKind};
use keybinds::KeybindResult;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use layout::kanata;
use mode::{ActiveMode, ModeResult};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

fn main() -> io::Result<()> {
    // Parse args
    let args: Vec<String> = std::env::args().collect();
    let split = args.iter().any(|a| a == "--split");
    let from_layout = flag_value(&args, "--from");
    let keyboard_flag = flag_value(&args, "-k").or_else(|| flag_value(&args, "--keyboard"));
    let layout_flag = flag_value(&args, "-l").or_else(|| flag_value(&args, "--layout"));

    // Collect indices of args consumed by flags
    let mut skip_indices = std::collections::HashSet::new();
    for (i, a) in args.iter().enumerate() {
        let a = a.as_str();
        if a == "--split" {
            skip_indices.insert(i);
        } else if matches!(a, "--from" | "-k" | "--keyboard" | "-l" | "--layout") {
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

    // Branch: if -k/-l were passed, use the data-driven grid path.
    // Otherwise use the legacy kanata path (unchanged behavior).
    let use_grid_path = keyboard_flag.is_some() || layout_flag.is_some();

    let mut ctx = if use_grid_path {
        build_grid_context(keyboard_flag.as_deref(), layout_flag.as_deref())?
    } else {
        build_kanata_context(&positional, split, from_layout.as_deref())?
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

    result
}

/// Parse `--flag <value>` style arguments. Returns the value if present.
fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string())
}

/// Build an [`AppContext`] via the new data-driven grid path. The legacy
/// `layout` field is still populated (for now, until modes migrate) using
/// a stub built from the grid's keyboard name so stats file paths stay
/// stable and existing code keeps compiling.
fn build_grid_context(keyboard: Option<&str>, layout: Option<&str>) -> io::Result<AppContext> {
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

    // Legacy `layout` field is unused on this path, but still required by
    // AppContext::new and the modes until the next session consolidates.
    let stub = layout::qwerty();
    let ctx = AppContext::new(stub, false, None).with_grid_manager(manager);
    Ok(ctx)
}

/// Build an [`AppContext`] via the original kanata config path.
fn build_kanata_context(
    positional: &[&String],
    split: bool,
    from_layout: Option<&str>,
) -> io::Result<AppContext> {
    let config_path = positional
        .first()
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            format!("{home}/lab/keywiz/layouts/gallium_v2.kbd")
        });

    let source = std::fs::read_to_string(&config_path).unwrap_or_else(|e| {
        eprintln!("Could not read {config_path}: {e}");
        eprintln!("Usage: keywiz [--split] [--from <layout>] [kanata-config-path] [layer-name]");
        eprintln!("   or: keywiz -k <keyboard> -l <layout>");
        std::process::exit(1);
    });

    let layer_name = positional
        .get(1)
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            kanata::first_layer_name(&source).unwrap_or_else(|| {
                eprintln!("No deflayer found in {config_path}");
                std::process::exit(1);
            })
        });

    let mut layout = kanata::parse_kanata(&source, &layer_name).unwrap_or_else(|| {
        eprintln!("Could not find layer '{layer_name}' in {config_path}");
        std::process::exit(1);
    });
    layout.set_colstag(split);

    let translate = from_layout.map(|from_name| {
        let from = if from_name == "qwerty" {
            layout::qwerty()
        } else {
            kanata::parse_kanata(&source, from_name).unwrap_or_else(|| {
                eprintln!(
                    "Could not find layout '{from_name}' (try 'qwerty' or a kanata layer name)"
                );
                std::process::exit(1);
            })
        };
        layout.translation_from(&from)
    });

    Ok(AppContext::new(layout, split, translate))
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
