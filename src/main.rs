mod app;
mod drill;
mod layout;
mod typing;
mod ui;

use std::io;

use app::{App, Mode};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use drill::{Drill, DrillLevel};
use layout::kanata;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use typing::TypingTest;

fn main() -> io::Result<()> {
    // Parse args
    let args: Vec<String> = std::env::args().collect();
    let split = args.iter().any(|a| a == "--split");
    let from_layout = args.iter()
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
    let positional: Vec<&String> = args.iter()
        .enumerate()
        .filter(|(i, _)| *i > 0 && !skip_indices.contains(i))
        .map(|(_, a)| a)
        .collect();

    let config_path = positional
        .first()
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            format!("{home}/.config/kanata/kanata.kbd")
        });

    let layer_name = positional
        .get(1)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "gallium_v2".into());

    let source = std::fs::read_to_string(&config_path)
        .unwrap_or_else(|e| {
            eprintln!("Could not read {config_path}: {e}");
            eprintln!("Usage: keywiz [--split] [--from qwerty] [kanata-config-path] [layer-name]");
            std::process::exit(1);
        });

    let layout = kanata::parse_kanata(&source, &layer_name)
        .unwrap_or_else(|| {
            eprintln!("Could not find layer '{layer_name}' in {config_path}");
            std::process::exit(1);
        });

    // Build translation map if --from is specified
    let translate = from_layout.map(|from_name| {
        let from = if from_name == "qwerty" {
            layout::qwerty()
        } else {
            // Try to parse it as a layer from the same kanata config
            kanata::parse_kanata(&source, from_name)
                .unwrap_or_else(|| {
                    eprintln!("Could not find layout '{from_name}' (try 'qwerty' or a kanata layer name)");
                    std::process::exit(1);
                })
        };
        layout.translation_from(&from)
    });

    // Setup terminal
    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let mut app = App::new(layout, split, translate);
    let mut drill: Option<Drill> = None;
    let mut typing_test: Option<TypingTest> = None;

    let result = run_loop(&mut terminal, &mut app, &mut drill, &mut typing_test);

    // Restore terminal — ignore errors (can fail over SSH)
    let _ = terminal::disable_raw_mode();
    let _ = io::stdout().execute(LeaveAlternateScreen);

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    drill: &mut Option<Drill>,
    typing_test: &mut Option<TypingTest>,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| {
            match app.mode {
                Mode::ModeSelect => ui::render_mode_select(f, app),
                Mode::Drill => {
                    if let Some(d) = drill.as_ref() {
                        ui::render_drill(f, d, app);
                    }
                }
                Mode::Typing => {
                    if let Some(t) = typing_test.as_ref() {
                        ui::render_typing(f, t, app);
                    }
                }
            }
        })?;

        if app.should_quit {
            return Ok(());
        }

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            match app.mode {
                Mode::ModeSelect => match key.code {
                    KeyCode::Esc => app.should_quit = true,
                    KeyCode::Char('1') => {
                        *drill = Some(Drill::new(&app.layout, DrillLevel::HomeRow));
                        app.mode = Mode::Drill;
                    }
                    KeyCode::Char('2') => {
                        *typing_test = Some(TypingTest::new(Some(20)));
                        app.mode = Mode::Typing;
                    }
                    KeyCode::Char('3') => {
                        *typing_test = Some(TypingTest::new(None));
                        app.mode = Mode::Typing;
                    }
                    _ => {}
                },
                Mode::Drill => match key.code {
                    KeyCode::Esc => {
                        app.mode = Mode::ModeSelect;
                        *drill = None;
                    }
                    KeyCode::Tab => {
                        app.show_keyboard = !app.show_keyboard;
                    }
                    KeyCode::BackTab => {
                        app.split = !app.split;
                    }
                    KeyCode::Char(ch) => {
                        let ch = app.translate_input(ch);
                        if let Some(d) = drill.as_mut() {
                            d.handle_input(ch, &app.layout);
                        }
                    }
                    _ => {}
                },
                Mode::Typing => match key.code {
                    KeyCode::Esc => {
                        app.mode = Mode::ModeSelect;
                        *typing_test = None;
                    }
                    KeyCode::Tab => {
                        app.show_keyboard = !app.show_keyboard;
                    }
                    KeyCode::BackTab => {
                        app.split = !app.split;
                    }
                    KeyCode::Char(ch) => {
                        let ch = app.translate_input(ch);
                        if let Some(t) = typing_test.as_mut() {
                            t.handle_input(ch);
                        }
                    }
                    _ => {}
                },
            }
        }
    }
}
