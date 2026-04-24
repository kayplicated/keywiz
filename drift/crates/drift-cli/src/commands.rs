//! Subcommand dispatch.
//!
//! Thin: builds a registry from stock analyzers, loads config
//! (preset or explicit path), materializes a pipeline, dispatches
//! to drift-score, renders with drift-report.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use drift_analyzer::Registry;
use drift_config::{DriftConfig, Override};
use drift_generate::SaConfig;
use drift_report::{Renderer, json::JsonRenderer, text::TextRenderer};
use drift_score::ScoreResult;
use serde::Serialize;

#[derive(Parser)]
#[command(name = "drift", about = "Flexion-aware keyboard layout scorer")]
struct Cli {
    /// Preset name (e.g. "neutral", "drifter"). Mutually exclusive
    /// with --config.
    #[arg(long, global = true, default_value = "neutral")]
    preset: String,
    /// Explicit config file path.
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    /// Override the config's corpus. Accepts `path` or `path:weight`
    /// per flag; repeat to blend multiple corpora. Any `--corpus`
    /// presence replaces the config's corpus entirely.
    ///
    /// Examples:
    ///   --corpus english.json
    ///   --corpus english.json:2 --corpus code.json:1
    #[arg(long, global = true)]
    corpus: Vec<String>,
    /// Keyboard JSON path. If omitted, the keyboard is picked per
    /// layout: `.dof` files carry a board descriptor that maps to
    /// `keyboards/ortho.json`, `keyboards/halcyon_elora.json`,
    /// or `keyboards/us_intl.json`; keywiz JSON5 layouts default
    /// to halcyon_elora.
    #[arg(long, global = true)]
    keyboard: Option<PathBuf>,
    /// Output format. Text is human-readable and colorized; JSON is
    /// a pretty-printed object for programmatic consumption.
    #[arg(long, global = true, value_enum, default_value_t = Format::Text)]
    format: Format,
    /// Override a scalar value in the loaded config. Dotted path is
    /// relative to `[analyzers]`. Repeatable.
    ///
    /// Examples:
    ///   --set sfb.penalty=-5.0
    ///   --set roll.adjacent_same_row_weight=0.2
    #[arg(long = "set", global = true, value_name = "KEY=VALUE")]
    set: Vec<String>,
    /// Enable an analyzer by name (adds to `[analyzers].enabled`
    /// with defaults). Repeatable.
    #[arg(long, global = true, value_name = "NAME")]
    enable: Vec<String>,
    /// Disable an analyzer by name (removes from
    /// `[analyzers].enabled`). Repeatable.
    #[arg(long, global = true, value_name = "NAME")]
    disable: Vec<String>,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum Format {
    Text,
    Json,
}

#[derive(Subcommand)]
enum Cmd {
    /// Score a single layout.
    Score { layout: PathBuf },
    /// Score two layouts and print a side-by-side total comparison.
    Compare {
        a: PathBuf,
        b: PathBuf,
        /// Also emit a per-key diff showing which chars moved between
        /// the two layouts.
        #[arg(long)]
        diff: bool,
    },
    /// Run simulated annealing from a seed layout and print the best
    /// layout found under the current pipeline.
    Generate {
        /// Seed layout to optimize.
        layout: PathBuf,
        /// Number of SA iterations.
        #[arg(long, default_value_t = 200_000)]
        iterations: usize,
        /// Starting temperature (higher = more exploratory early).
        #[arg(long, default_value_t = 5.0)]
        temp_start: f64,
        /// Ending temperature (near 0 for a greedy finish).
        #[arg(long, default_value_t = 0.01)]
        temp_end: f64,
        /// Characters pinned in place — comma-separated (e.g. "q,z,'").
        #[arg(long, default_value = "")]
        pin: String,
        /// Fixed RNG seed for reproducible runs.
        #[arg(long)]
        rng_seed: Option<u64>,
        /// Write the result as a keywiz JSON5 layout file.
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

/// Run the drift CLI against the process's own argv.
pub fn dispatch() -> Result<()> {
    dispatch_args(std::env::args())
}

/// Run the drift CLI against a caller-supplied arg list.
///
/// Used by external entrypoints (e.g. keywiz's `--drift`
/// passthrough) to forward their argv into drift without shelling
/// out. `args` must include the program name at position 0 — clap
/// treats the first element as `argv[0]` by convention.
pub fn dispatch_args<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::try_parse_from(args)?;

    let mut registry = Registry::new();
    drift_analyzers::register_all(&mut registry);

    let mut config = load_config(&cli)?;
    let overrides = collect_overrides(&cli).context("parsing --set/--enable/--disable flags")?;
    drift_config::apply_overrides(&mut config, &overrides)
        .context("applying CLI overrides to config")?;
    let pipeline = drift_config::build_pipeline(&config, &registry)
        .context("building analyzer pipeline")?;

    let mut corpus = resolve_corpus(&cli.corpus, &config)
        .context("resolving corpus from --corpus / preset")?;

    // If any analyzer scopes at Ngram(n >= 4), make sure the corpus
    // has a derived table for that n. Derivation is explicit rather
    // than lazy — the caller decides what compute to pay for.
    if let Some(max_n) = pipeline
        .scopes()
        .into_iter()
        .filter_map(|s| match s {
            drift_core::Scope::Ngram(n) if n >= 4 => Some(n),
            _ => None,
        })
        .max()
    {
        corpus
            .ensure_ngrams(max_n)
            .with_context(|| format!("deriving n-gram data up to n={max_n}"))?;
    }

    let renderer: Box<dyn Renderer> = match cli.format {
        Format::Text => Box::new(TextRenderer),
        Format::Json => Box::new(JsonRenderer),
    };

    match cli.command {
        Cmd::Score { layout } => {
            let keyboard = load_keyboard_for(cli.keyboard.as_deref(), &layout)?;
            let layout = load_layout_any(&layout, &keyboard)?;
            let result = drift_score::run(&pipeline, &layout, &keyboard, &corpus);
            println!("{}", renderer.render(&result));
        }
        Cmd::Compare { a, b, diff } => {
            let keyboard = load_keyboard_for(cli.keyboard.as_deref(), &a)?;
            let la = load_layout_any(&a, &keyboard)?;
            let lb = load_layout_any(&b, &keyboard)?;
            let ra = drift_score::run(&pipeline, &la, &keyboard, &corpus);
            let rb = drift_score::run(&pipeline, &lb, &keyboard, &corpus);
            let diff_entries = if diff {
                Some(drift_report::diff::compute::diff(&la, &lb, &keyboard))
            } else {
                None
            };
            match cli.format {
                Format::Text => {
                    println!("{}", renderer.render(&ra));
                    println!("---");
                    println!("{}", renderer.render(&rb));
                    println!(
                        "\nDelta ({} - {}): {:+.3}",
                        ra.layout_name,
                        rb.layout_name,
                        ra.total - rb.total
                    );
                    if let Some(entries) = &diff_entries {
                        println!();
                        println!(
                            "{}",
                            drift_report::diff::text::render(
                                entries,
                                &ra.layout_name,
                                &rb.layout_name,
                            )
                        );
                    }
                }
                Format::Json => {
                    let delta = ra.total - rb.total;
                    let diff_payload = diff_entries.as_ref().map(|entries| {
                        drift_report::diff::json::payload(
                            entries,
                            &ra.layout_name,
                            &rb.layout_name,
                        )
                    });
                    let envelope = CompareJson {
                        a: &ra,
                        b: &rb,
                        delta,
                        diff: diff_payload,
                    };
                    println!("{}", serde_json::to_string_pretty(&envelope)?);
                }
            }
        }
        Cmd::Generate {
            layout,
            iterations,
            temp_start,
            temp_end,
            pin,
            rng_seed,
            output,
        } => {
            let keyboard = load_keyboard_for(cli.keyboard.as_deref(), &layout)?;
            let seed = load_layout_any(&layout, &keyboard)?;
            let pinned: Vec<char> = pin
                .split(',')
                .filter_map(|s| s.trim().chars().next())
                .collect();
            let sa = SaConfig {
                iterations,
                temp_start,
                temp_end,
                pinned,
                seed: rng_seed,
            };
            let result = drift_generate::generate(&pipeline, &keyboard, &corpus, seed, &sa)?;
            let fresh = drift_score::run(&pipeline, &result.best, &keyboard, &corpus);

            match cli.format {
                Format::Text => {
                    println!("Initial:  {:+.3}", result.initial_score);
                    println!(
                        "Best:     {:+.3}  ({} accepted of {} iterations, {} uphill)",
                        result.best_score,
                        result.accepted,
                        result.iterations,
                        result.uphill_accepted
                    );
                    println!();
                    println!("{}", renderer.render(&fresh));
                }
                Format::Json => {
                    let envelope = GenerateJson {
                        initial_score: result.initial_score,
                        best_score: result.best_score,
                        iterations: result.iterations,
                        accepted: result.accepted,
                        uphill_accepted: result.uphill_accepted,
                        result: &fresh,
                    };
                    println!("{}", serde_json::to_string_pretty(&envelope)?);
                }
            }

            if let Some(path) = output {
                drift_keyboard::writer::write(&path, &result.best, None)
                    .with_context(|| format!("writing generated layout {}", path.display()))?;
                if matches!(cli.format, Format::Text) {
                    println!("\nWrote generated layout to {}", path.display());
                }
            } else if matches!(cli.format, Format::Text) {
                println!("\n(pass --output <path> to save the result as JSON5)");
            }
        }
    }
    Ok(())
}

/// JSON wrapper for `compare`. Emits both scored results plus the
/// `a - b` total delta in one object. `diff` is present only when
/// `--diff` was passed.
#[derive(Serialize)]
struct CompareJson<'a> {
    a: &'a ScoreResult,
    b: &'a ScoreResult,
    delta: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    diff: Option<drift_report::diff::json::DiffPayload<'a>>,
}

/// JSON wrapper for `generate`. Carries SA metadata alongside the
/// rescored best layout.
#[derive(Serialize)]
struct GenerateJson<'a> {
    initial_score: f64,
    best_score: f64,
    iterations: usize,
    accepted: usize,
    uphill_accepted: usize,
    result: &'a ScoreResult,
}

fn load_config(cli: &Cli) -> Result<DriftConfig> {
    if let Some(path) = &cli.config {
        drift_config::load(path).with_context(|| format!("loading config {}", path.display()))
    } else {
        drift_config::load_preset(&cli.preset)
            .with_context(|| format!("loading preset {:?}", cli.preset))
    }
}

/// Gather `--set`, `--enable`, `--disable` flags into a single
/// ordered list of overrides. `--set` lands first so it can seed
/// weights before `--enable` adds a freshly-weighted analyzer.
fn collect_overrides(cli: &Cli) -> Result<Vec<Override>> {
    let mut out = Vec::with_capacity(cli.set.len() + cli.enable.len() + cli.disable.len());
    for raw in &cli.set {
        let (path, value) = raw
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("--set expects KEY=VALUE, got {raw:?}"))?;
        out.push(Override::Set {
            path: path.to_string(),
            value: value.to_string(),
        });
    }
    for name in &cli.enable {
        out.push(Override::Enable(name.clone()));
    }
    for name in &cli.disable {
        out.push(Override::Disable(name.clone()));
    }
    Ok(out)
}

/// Load the appropriate keyboard for a given layout.
///
/// Priority:
/// 1. Explicit `--keyboard` overrides everything and is used verbatim.
/// 2. `.dof` layouts peek their `board` field and pick the default
///    keyboard for that board (via `drift_dof::default_keyboard_path`).
/// 3. keywiz JSON5 layouts fall back to `keyboards/halcyon_elora.json`.
///
/// In cases 2 and 3 the hint is a relative path like
/// `keyboards/foo.json`; it is resolved via [`resolve_keyboard_hint`]
/// so it works regardless of the caller's current working directory.
fn load_keyboard_for(
    explicit: Option<&std::path::Path>,
    layout_path: &std::path::Path,
) -> Result<drift_core::Keyboard> {
    let path: PathBuf = if let Some(p) = explicit {
        p.to_path_buf()
    } else if is_dof_path(layout_path) {
        let doc = drift_dof::parse::load(layout_path)
            .with_context(|| format!("peeking .dof for board: {}", layout_path.display()))?;
        let hint = drift_dof::default_keyboard_path(&doc.board).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown .dof board {:?}; pass --keyboard to override",
                doc.board
            )
        })?;
        resolve_keyboard_hint(hint, layout_path)?
    } else {
        resolve_keyboard_hint("keyboards/halcyon_elora.json", layout_path)?
    };
    drift_keyboard::load_keyboard(&path)
        .with_context(|| format!("loading keyboard {}", path.display()))
}

/// Resolve a default-keyboard hint like `keyboards/foo.json` into an
/// absolute path that actually exists on disk.
///
/// Hints come from two places: the `.dof` board→keyboard mapping in
/// `drift-dof`, and the hardcoded JSON5 fallback above. Both are
/// repo-relative, which means they only work when drift is invoked
/// from the keywiz repo root. Since `keywiz --drift` and `drift`
/// invoked from anywhere else are both legitimate entry points, we
/// search for the hint in a handful of likely locations instead of
/// relying on cwd.
///
/// Search order:
/// 1. If the hint is absolute and exists, use it verbatim.
/// 2. Walk up from the layout file's directory looking for a sibling
///    `keyboards/` (or whatever the hint's first segment is). This
///    handles `drift score /abs/path/to/layouts/foo.json` cleanly.
/// 3. Walk up from the current working directory the same way. This
///    preserves the classic `cd keywiz && drift ...` workflow.
///
/// If nothing matches, return an error listing what was searched and
/// suggesting `--keyboard`, so the user sees *why* it failed instead
/// of a bare `No such file or directory`.
fn resolve_keyboard_hint(hint: &str, layout_path: &std::path::Path) -> Result<PathBuf> {
    let hint_path = std::path::Path::new(hint);
    if hint_path.is_absolute() {
        if hint_path.exists() {
            return Ok(hint_path.to_path_buf());
        }
        return Err(anyhow::anyhow!(
            "default keyboard {:?} not found; pass --keyboard to override",
            hint
        ));
    }

    let layout_abs = std::fs::canonicalize(layout_path).unwrap_or_else(|_| layout_path.to_path_buf());
    let layout_start = layout_abs.parent().map(|p| p.to_path_buf());
    let cwd_start = std::env::current_dir().ok();

    let mut searched: Vec<PathBuf> = Vec::new();
    for start in [layout_start, cwd_start].into_iter().flatten() {
        for dir in start.ancestors() {
            let candidate = dir.join(hint_path);
            if candidate.is_file() {
                return Ok(candidate);
            }
            if !searched.contains(&candidate) {
                searched.push(candidate);
            }
        }
    }

    let searched_str = searched
        .iter()
        .map(|p| format!("  {}", p.display()))
        .collect::<Vec<_>>()
        .join("\n");
    Err(anyhow::anyhow!(
        "default keyboard {:?} not found on disk; searched:\n{}\npass --keyboard to override",
        hint,
        searched_str
    ))
}

/// Load a layout, picking the `.dof` or keywiz JSON5 reader based
/// on the file extension.
fn load_layout_any(
    path: &std::path::Path,
    keyboard: &drift_core::Keyboard,
) -> Result<drift_core::Layout> {
    if is_dof_path(path) {
        drift_dof::load_layout(path, keyboard)
            .with_context(|| format!("loading .dof layout {}", path.display()))
    } else {
        drift_keyboard::load_layout(path, keyboard)
            .with_context(|| format!("loading layout {}", path.display()))
    }
}

fn is_dof_path(path: &std::path::Path) -> bool {
    path.extension().and_then(|s| s.to_str()) == Some("dof")
}

/// Resolve the corpus source from CLI arguments + config. Supports
/// a single corpus (the preset default, or a single `--corpus`
/// flag), or a blended corpus when multiple `--corpus` flags are
/// passed.
fn resolve_corpus(
    flags: &[String],
    config: &DriftConfig,
) -> Result<drift_corpus::MemoryCorpus> {
    if flags.is_empty() {
        return drift_corpus::load(&config.corpus_path).with_context(|| {
            format!("loading corpus {}", config.corpus_path.display())
        });
    }

    // Parse each flag into (path, weight). Default weight is 1.0
    // when `:weight` is omitted.
    let mut parsed: Vec<(PathBuf, f64)> = Vec::with_capacity(flags.len());
    for raw in flags {
        let (path_part, weight) = match raw.rsplit_once(':') {
            // If the RHS parses as f64 we treat it as a weight;
            // otherwise it's likely part of the path (Windows drive
            // letters, colons in filenames) and the whole string is
            // the path.
            Some((lhs, rhs)) if rhs.parse::<f64>().is_ok() => {
                (lhs.to_string(), rhs.parse::<f64>().unwrap())
            }
            _ => (raw.clone(), 1.0),
        };
        if weight <= 0.0 {
            anyhow::bail!(
                "corpus weight must be positive, got {weight} for {raw:?}"
            );
        }
        parsed.push((PathBuf::from(path_part), weight));
    }

    // Single path → load directly, no blend overhead.
    if parsed.len() == 1 {
        let (path, _) = &parsed[0];
        return drift_corpus::load(path)
            .with_context(|| format!("loading corpus {}", path.display()));
    }

    // Multiple paths → load each and blend by weight. Downcast to
    // `Box<dyn CorpusSource>` for the blend API.
    use drift_core::CorpusSource;
    let mut sources: Vec<(Box<dyn CorpusSource>, f64)> = Vec::with_capacity(parsed.len());
    for (path, weight) in &parsed {
        let corpus = drift_corpus::load(path)
            .with_context(|| format!("loading corpus {}", path.display()))?;
        sources.push((Box::new(corpus), *weight));
    }
    drift_corpus::blend(&sources).context("blending corpora")
}

