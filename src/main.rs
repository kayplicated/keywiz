mod app;
mod config;
mod engine;
mod layout;
mod mode;
mod stats;
mod ui;
mod words;

use std::io;

use app::AppContext;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
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
    let from_layout = args
        .iter()
        .position(|a| a == "--from")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str());
    // Collect indices of args consumed by flags
    let mut skip_indices = std::collections::HashSet::new();
    for (i, a) in args.iter().enumerate() {
        if a == "--split" {
            skip_indices.insert(i);
        } else if a == "--from" {
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

    // Build translation map if --from is specified
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

    // Setup terminal
    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let mut ctx = AppContext::new(layout, split, translate);
    ctx.stats.set_persistent(stats::persist::load(&ctx.layout.name));

    let result = run_loop(&mut terminal, &mut ctx);

    // Restore terminal — ignore errors (can fail over SSH)
    let _ = terminal::disable_raw_mode();
    let _ = io::stdout().execute(LeaveAlternateScreen);

    stats::persist::save(&ctx.layout.name, ctx.stats.persistent());

    result
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

            // Shared key handling — done once, not per mode
            match key.code {
                KeyCode::Tab => {
                    ctx.show_keyboard = !ctx.show_keyboard;
                    continue;
                }
                KeyCode::BackTab => {
                    ctx.split = !ctx.split;
                    ctx.layout.set_colstag(ctx.split);
                    continue;
                }
                KeyCode::F(2) => {
                    ctx.show_heatmap = !ctx.show_heatmap;
                    continue;
                }
                _ => {}
            }

            // Mode-specific handling
            match mode.handle_input(key, ctx) {
                ModeResult::Stay => {}
                ModeResult::Quit => return Ok(()),
                ModeResult::SwitchTo(new_mode) => {
                    // Save when returning to menu (session boundary).
                    if matches!(new_mode, ActiveMode::Select(_)) {
                        stats::persist::save(&ctx.layout.name, ctx.stats.persistent());
                    }
                    mode = new_mode;
                }
            }
        }
    }
}
