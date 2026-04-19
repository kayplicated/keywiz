mod app;
mod config;
mod engine;
mod integrations;
mod keybinds;
mod keyboard;
mod mapping;
mod mode;
mod prefs;
mod renderer;
mod stats;
mod typing;
mod words;

use std::io;

use app::AppContext;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use keybinds::KeybindResult;
use mode::{ActiveMode, ModeResult};
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let from_layout = flag_value(&args, "--from");
    let keyboard_flag = flag_value(&args, "-k").or_else(|| flag_value(&args, "--keyboard"));
    let layout_flag = flag_value(&args, "-l").or_else(|| flag_value(&args, "--layout"));
    let kanata_path = flag_value(&args, "--kanata");

    if kanata_path.is_some() {
        eprintln!(
            "keywiz: --kanata is temporarily disabled while the engine is \
             being restructured. Use -k and -l with JSON5 keyboards/layouts \
             for now."
        );
        std::process::exit(1);
    }

    // Validate --from up front so an unknown layout fails fast with a
    // clear message; runtime rebuilds trust the name is loadable.
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
    let mut ctx = build_context(keyboard, layout, from_layout.as_deref())?;

    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    ctx.stats
        .set_persistent(stats::persist::load(ctx.stats_key()));

    let result = run_loop(&mut terminal, &mut ctx);

    let _ = terminal::disable_raw_mode();
    let _ = io::stdout().execute(LeaveAlternateScreen);

    stats::persist::save(ctx.stats_key(), ctx.stats.persistent());
    prefs::Prefs::save(ctx.engine.current_keyboard(), ctx.stats_key());

    result
}

fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string())
}

fn build_context(
    keyboard: Option<&str>,
    layout: Option<&str>,
    from_layout: Option<&str>,
) -> io::Result<AppContext> {
    let mut engine = engine::Engine::new().unwrap_or_else(|e| {
        eprintln!("keywiz: could not load keyboards/layouts: {e}");
        std::process::exit(1);
    });
    if let Some(name) = keyboard
        && let Err(e) = engine.set_keyboard(name)
    {
        eprintln!("keywiz: {e}");
        std::process::exit(1);
    }
    if let Some(name) = layout
        && let Err(e) = engine.set_layout(name)
    {
        eprintln!("keywiz: {e}");
        std::process::exit(1);
    }

    Ok(AppContext::new(engine, from_layout.map(str::to_string)))
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

            if matches!(keybinds::handle_shared(key, ctx), KeybindResult::Handled) {
                continue;
            }

            match mode.handle_input(key, ctx) {
                ModeResult::Stay => {}
                ModeResult::Quit => return Ok(()),
                ModeResult::SwitchTo(new_mode) => {
                    if matches!(new_mode, ActiveMode::Select(_)) {
                        stats::persist::save(ctx.stats_key(), ctx.stats.persistent());
                    }
                    mode = new_mode;
                }
            }
        }
    }
}
