mod engine;
mod exercise;
mod integrations;
mod keybinds;
mod keyboard;
mod mapping;
mod prefs;
mod renderer;
mod stats_adapter;
mod words;

use std::io;

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::ExecutableCommand;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

use engine::Engine;
use keybinds::{classify, Classified};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // --drift forwards the rest of argv straight to the drift CLI.
    // Runs in-process (shared workspace) rather than shelling out,
    // so `keywiz --drift score foo.json` is bit-identical to
    // `drift score foo.json`.
    if args.iter().any(|a| a == "--drift") {
        // Forward argv with --drift stripped. Replace argv[0] with
        // "drift" so clap's usage/error messages read naturally
        // (`Usage: drift [OPTIONS] ...`) instead of leaking the
        // keywiz binary name.
        let mut forward: Vec<String> = Vec::with_capacity(args.len());
        forward.push("drift".to_string());
        forward.extend(
            args.iter()
                .skip(1)
                .filter(|a| a.as_str() != "--drift")
                .cloned(),
        );
        if let Err(e) = drift_cli::dispatch_args(forward) {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
        return Ok(());
    }

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
    let overlay = saved.overlay.as_deref();

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
        engine.set_exercise_from_pref(name);
    }
    if let Some(name) = overlay {
        engine.set_overlay_by_name(name);
    }

    terminal::enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let result = run_loop(&mut terminal, &mut engine);

    let _ = terminal::disable_raw_mode();
    let _ = io::stdout().execute(LeaveAlternateScreen);

    engine.end_events_session();
    prefs::Prefs::save(
        engine.current_keyboard(),
        engine.current_layout(),
        &engine.current_exercise(),
        engine.active_overlay().name(),
    );

    result
}

fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(|s| s.to_string())
}

/// Animation tick for the flash layer — we redraw at this
/// interval while a flash is within the fade window so the user
/// sees the color step down between keystrokes. Outside the fade
/// window the loop goes back to blocking reads.
const FLASH_POLL_MS: u64 = 40;
/// Max age (ms) we keep polling for flash animation. Must track
/// `FLASH_FADE_MS` in the renderer; past that age the flash is
/// invisible anyway, so there's no reason to keep redrawing.
const FLASH_ANIMATE_MS: u64 = 300;

/// True when the engine's flash layer is on and the most recent
/// keystroke is young enough that the renderer will still paint
/// it. Drives the poll-vs-block decision in the main loop.
fn flash_is_animating(engine: &Engine) -> bool {
    if !engine.flash_enabled() {
        return false;
    }
    let Some(flash) = engine.last_flash() else {
        return false;
    };
    flash.at.elapsed().as_millis() as u64 <= FLASH_ANIMATE_MS
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    engine: &mut Engine,
) -> io::Result<()> {
    loop {
        let placements = engine.placements_for_terminal();
        let display = engine.display_state();
        let overlay = engine.active_overlay();
        terminal.draw(|f| {
            renderer::terminal::draw_frame(f, &placements, &display, overlay, engine)
        })?;

        // Poll-or-block: while flash is active *and* the last
        // keystroke is still inside the fade window, poll on a
        // short timeout so the fade animates. Otherwise block
        // indefinitely — zero CPU use between keystrokes.
        let key_opt = if flash_is_animating(engine) {
            if event::poll(std::time::Duration::from_millis(FLASH_POLL_MS))? {
                match event::read()? {
                    Event::Key(k) => Some(k),
                    _ => continue,
                }
            } else {
                // Poll timed out — loop back to redraw with the
                // new (larger) flash age.
                continue;
            }
        } else {
            match event::read()? {
                Event::Key(k) => Some(k),
                _ => continue,
            }
        };
        if let Some(key) = key_opt {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let classified = classify(key);

            // F1 help modal: Esc/F1 closes, everything else
            // swallowed. Stateless reference; no arrow navigation.
            if engine.help_page_visible() {
                if matches!(
                    classified,
                    Classified::Quit | Classified::ToggleHelpPage
                ) {
                    engine.toggle_help_page();
                }
                continue;
            }

            // F5 layout-iterations modal: Esc/F5 closes, otherwise
            // the page is read-only (data scope comes from F4's
            // stats filter). Typing is swallowed.
            if engine.layout_page_visible() {
                if matches!(
                    classified,
                    Classified::Quit | Classified::ToggleLayoutPage
                ) {
                    engine.toggle_layout_page();
                }
                continue;
            }

            // F4 stats modal: arrows retarget to filter + page
            // cycling. Esc/F4 closes the modal instead of quitting.
            // Typing is swallowed entirely.
            //
            // Keybinds in the modal:
            //   Ctrl+←/→  cycle (layout, keyboard) combo — combos
            //             that actually exist in the event store
            //             plus an "all combos" sentinel.
            //   Ctrl+↑/↓  cycle pages (Overview / Progression /
            //             Layout × You).
            //   Alt+↑/↓   cycle time granularity (current session /
            //             day / week / month / year / all).
            //   Alt+←/→   walk backward/forward through the active
            //             granularity's buckets.
            if engine.stats_page_visible() {
                match classified {
                    Classified::Quit | Classified::ToggleStatsPage => {
                        engine.toggle_stats_page()
                    }
                    Classified::NextLayout => engine.stats_next_combo(),
                    Classified::PrevLayout => engine.stats_prev_combo(),
                    Classified::NextKeyboard => engine.next_stats_view(),
                    Classified::PrevKeyboard => engine.prev_stats_view(),
                    Classified::NextExerciseCategory => {
                        engine.stats_next_granularity()
                    }
                    Classified::PrevExerciseCategory => {
                        engine.stats_prev_granularity()
                    }
                    Classified::NextExerciseInstance => {
                        engine.stats_older_offset()
                    }
                    Classified::PrevExerciseInstance => {
                        engine.stats_newer_offset()
                    }
                    _ => {}
                }
                continue;
            }

            match classified {
                Classified::Quit => return Ok(()),
                Classified::Typing(ch) => {
                    engine.process_input(ch);
                }
                Classified::ToggleSlot => engine.toggle_slot_visible(),
                Classified::ToggleFlash => engine.toggle_flash(),
                Classified::CycleSlot => engine.cycle_slot(),
                Classified::ToggleHeatmap => engine.cycle_overlay(),
                Classified::ToggleHelpPage => engine.toggle_help_page(),
                Classified::ToggleStatsPage => engine.toggle_stats_page(),
                Classified::ToggleLayoutPage => engine.toggle_layout_page(),
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
                Classified::NextExerciseCategory => {
                    engine.next_exercise_category();
                }
                Classified::PrevExerciseCategory => {
                    engine.prev_exercise_category();
                }
                Classified::NextExerciseInstance => {
                    engine.next_exercise_instance();
                }
                Classified::PrevExerciseInstance => {
                    engine.prev_exercise_instance();
                }
                Classified::Ignored => {}
            }
        }
    }
}

