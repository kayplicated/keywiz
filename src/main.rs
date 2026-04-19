mod config;
mod engine;
mod exercise;
mod integrations;
mod keyboard;
mod mapping;
mod prefs;
mod renderer;
mod stats;
mod words;

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

use engine::Engine;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let from_layout = flag_value(&args, "--from");
    let keyboard_flag = flag_value(&args, "-k").or_else(|| flag_value(&args, "--keyboard"));
    let layout_flag = flag_value(&args, "-l").or_else(|| flag_value(&args, "--layout"));
    let kanata_path = flag_value(&args, "--kanata");

    if kanata_path.is_some() {
        eprintln!(
            "keywiz: --kanata is disabled while the kanata integration is \
             being ported. Use -k / -l with JSON5 keyboards and layouts."
        );
        std::process::exit(1);
    }

    // Pre-validate --from.
    if let Some(name) = from_layout.as_deref() {
        let path = std::path::Path::new("layouts").join(format!("{name}.json"));
        if let Err(e) = mapping::loader::load(&path) {
            eprintln!("keywiz: --from {name}: {e}");
            std::process::exit(1);
        }
    }

    let saved = prefs::Prefs::load();
    let keyboard = keyboard_flag.as_deref().or(saved.keyboard.as_deref());
    let layout = layout_flag.as_deref().or(saved.layout.as_deref());
    let exercise = saved.exercise.as_deref();

    let mut engine = Engine::new(from_layout).unwrap_or_else(|e| {
        eprintln!("keywiz: could not load keyboards/layouts: {e}");
        std::process::exit(1);
    });
    if let Some(name) = keyboard
        && let Err(e) = engine.set_keyboard(name)
    {
        eprintln!("keywiz: {e}");
    }
    if let Some(name) = layout
        && let Err(e) = engine.set_layout(name)
    {
        eprintln!("keywiz: {e}");
    }
    if let Some(name) = exercise {
        engine.set_exercise(name);
    }

    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let result = run_loop(&mut terminal, &mut engine);

    let _ = terminal::disable_raw_mode();
    let _ = io::stdout().execute(LeaveAlternateScreen);

    engine.persist_stats();
    prefs::Prefs::save(
        engine.current_keyboard(),
        engine.current_layout(),
        engine.current_exercise(),
    );

    result
}

fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string())
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    engine: &mut Engine,
) -> io::Result<()> {
    loop {
        let placements = engine.placements_for_terminal();
        let display = engine.display_state();
        terminal.draw(|f| renderer::terminal::draw_frame(f, &placements, &display))?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match classify(key) {
                Classified::Quit => return Ok(()),
                Classified::Typing(ch) => {
                    engine.process_input(ch);
                }
                Classified::ToggleKeyboard => engine.toggle_keyboard_visible(),
                Classified::ToggleHeatmap => engine.toggle_heatmap(),
                Classified::NextKeyboard => {
                    let _ = engine.next_keyboard();
                }
                Classified::PrevKeyboard => {
                    let _ = engine.prev_keyboard();
                }
                Classified::NextLayout => {
                    let _ = engine.next_layout();
                }
                Classified::PrevLayout => {
                    let _ = engine.prev_layout();
                }
                Classified::NextExercise => {
                    engine.persist_stats();
                    engine.next_exercise();
                }
                Classified::PrevExercise => {
                    engine.persist_stats();
                    engine.prev_exercise();
                }
                Classified::Control(k) => {
                    // Let the active exercise consume arrow keys etc.
                    engine.handle_exercise_control(k);
                }
            }
        }
    }
}

enum Classified {
    Quit,
    Typing(char),
    ToggleKeyboard,
    ToggleHeatmap,
    NextKeyboard,
    PrevKeyboard,
    NextLayout,
    PrevLayout,
    NextExercise,
    PrevExercise,
    Control(KeyEvent),
}

fn classify(key: KeyEvent) -> Classified {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);

    match key.code {
        KeyCode::Esc => Classified::Quit,
        KeyCode::Tab => Classified::ToggleKeyboard,
        KeyCode::F(2) => Classified::ToggleHeatmap,
        KeyCode::Up if ctrl => Classified::PrevKeyboard,
        KeyCode::Down if ctrl => Classified::NextKeyboard,
        KeyCode::Left if ctrl => Classified::PrevLayout,
        KeyCode::Right if ctrl => Classified::NextLayout,
        KeyCode::Left if alt => Classified::PrevExercise,
        KeyCode::Right if alt => Classified::NextExercise,
        KeyCode::Char(ch) => Classified::Typing(ch),
        KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => Classified::Control(key),
        _ => Classified::Control(key),
    }
}
