//! Command-line interface for drift.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::config::Config;
use crate::corpus::Corpus;
use crate::keyboard::Keyboard;
use crate::layout::Layout;
use crate::report;
use crate::score;

/// drift — flexion-aware keyboard layout scorer.
#[derive(Debug, Parser)]
#[command(name = "drift", version, about)]
struct Cli {
    /// Path to a drift.toml config. Defaults to the one shipped
    /// with the crate.
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Corpus JSON path(s). May be passed multiple times; each one
    /// produces its own score report. Defaults to config's corpus.
    /// Mutually exclusive with `--blend`.
    #[arg(long, short = 'c', global = true, conflicts_with = "blend")]
    corpus: Vec<PathBuf>,

    /// Blended corpora as `path:weight,path:weight,...`. The blend
    /// is computed at load time and scored as a single synthetic
    /// corpus. Weights are normalized; e.g. `a.json:2,b.json:1` makes
    /// a contribute 2/3 of the result. Mutually exclusive with
    /// `--corpus`.
    #[arg(long, global = true, value_delimiter = ',')]
    blend: Vec<String>,

    /// Keyboard JSON5 path. Defaults to keywiz's halcyon_elora_v2.
    #[arg(long, short = 'k', global = true)]
    keyboard: Option<PathBuf>,

    /// Output machine-readable JSON instead of the text report.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Score a single layout.
    Score {
        /// Path to a keywiz layout JSON5 file.
        layout: PathBuf,
    },
    /// Compare two layouts side-by-side.
    Compare {
        layout_a: PathBuf,
        layout_b: PathBuf,
    },
    /// Generate a layout via simulated annealing from a seed layout.
    Generate {
        /// Seed layout. The generator starts here and mutates.
        seed: PathBuf,
        /// Characters to pin in their seed positions (never swapped).
        /// E.g. `-p nrtsghaei` to lock in gallium home row.
        #[arg(long, short = 'p', default_value = "")]
        pin: String,
        /// Number of SA iterations.
        #[arg(long, short = 'n', default_value_t = 20000)]
        iterations: usize,
        /// Starting temperature.
        #[arg(long, default_value_t = 5.0)]
        temp_start: f64,
        /// Ending temperature.
        #[arg(long, default_value_t = 0.01)]
        temp_end: f64,
        /// Deterministic RNG seed.
        #[arg(long)]
        seed_rng: Option<u64>,
        /// If set, write the best layout out as JSON5 to this path.
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },
}

/// Run the CLI. Called from `main`.
pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let config = load_config(cli.config.as_deref())?;
    let corpora = load_corpora(&cli.corpus, &cli.blend, &config)?;
    let keyboard = load_keyboard(cli.keyboard.as_deref())?;

    // Build the trigram pipeline once for the whole run.
    let pipeline = match config.trigram.as_ref() {
        Some(tbl) => crate::trigram::build_pipeline(tbl)?,
        None => crate::trigram::TrigramPipeline::empty(),
    };

    match cli.cmd {
        Command::Score { layout } => {
            let layout = Layout::load(&layout, &keyboard)?;
            let reports: Vec<_> = corpora
                .iter()
                .map(|c| score::score(
                    &layout, &keyboard, c, &config, &pipeline, score::ScoreMode::Full,
                ))
                .collect();
            emit_score(&reports, cli.json)?;
        }
        Command::Compare { layout_a, layout_b } => {
            let a = Layout::load(&layout_a, &keyboard)?;
            let b = Layout::load(&layout_b, &keyboard)?;
            let pairs: Vec<_> = corpora
                .iter()
                .map(|c| {
                    (
                        score::score(
                            &a, &keyboard, c, &config, &pipeline, score::ScoreMode::Full,
                        ),
                        score::score(
                            &b, &keyboard, c, &config, &pipeline, score::ScoreMode::Full,
                        ),
                    )
                })
                .collect();
            emit_compare(&pairs, cli.json)?;
        }
        Command::Generate {
            seed,
            pin,
            iterations,
            temp_start,
            temp_end,
            seed_rng,
            output,
        } => {
            if corpora.len() > 1 {
                anyhow::bail!(
                    "generate requires a single corpus; use `--blend` for \
                     blended scoring or pass only one `-c`"
                );
            }
            let seed_layout = crate::layout::Layout::load(&seed, &keyboard)?;
            let opts = crate::generate::GenerateOptions {
                iterations,
                temp_start,
                temp_end,
                pinned: pin.chars().collect(),
                seed: seed_rng,
            };
            let result = crate::generate::generate(
                &seed_layout,
                &keyboard,
                &corpora[0],
                &config,
                &pipeline,
                &opts,
            );
            emit_generate(
                &result,
                &keyboard,
                &corpora[0],
                &config,
                &pipeline,
                cli.json,
                output.as_deref(),
            )?;
        }
    }

    Ok(())
}

/// Present a generate result. Always prints a short summary; if
/// `--output` is supplied, the best layout is written out in keywiz
/// JSON5 format.
fn emit_generate(
    result: &crate::generate::GenerateResult,
    keyboard: &crate::keyboard::Keyboard,
    corpus: &crate::corpus::Corpus,
    config: &Config,
    pipeline: &crate::trigram::TrigramPipeline,
    json: bool,
    output: Option<&std::path::Path>,
) -> Result<()> {
    // Score the best layout with full detail for the report path.
    let best_report =
        score::score(&result.best, keyboard, corpus, config, pipeline, score::ScoreMode::Full);

    if json {
        #[derive(serde::Serialize)]
        struct Out<'a> {
            initial_score: f64,
            best_score: f64,
            iterations: usize,
            accepted: usize,
            uphill_accepted: usize,
            best: &'a score::ScoreReport,
        }
        let out = Out {
            initial_score: result.initial_score,
            best_score: result.best_score,
            iterations: result.iterations,
            accepted: result.accepted,
            uphill_accepted: result.uphill_accepted,
            best: &best_report,
        };
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        use owo_colors::OwoColorize;
        println!("{}", "Generate run:".bold());
        println!(
            "  iterations:       {}",
            result.iterations
        );
        println!(
            "  accepted swaps:   {} ({} uphill)",
            result.accepted, result.uphill_accepted
        );
        println!(
            "  initial score:    {:.3}",
            result.initial_score
        );
        println!(
            "  best score:       {:.3} ({:+.3})",
            result.best_score.bright_green(),
            result.best_score - result.initial_score
        );
        println!();
        crate::report::print(&best_report);
    }

    if let Some(path) = output {
        write_layout_json5(&result.best, path)?;
        eprintln!("wrote {}", path.display());
    }

    Ok(())
}

/// Serialize a [`Layout`] back into keywiz JSON5 form. Walks the
/// keyboard's alpha-core key ids and emits the char bound at each.
fn write_layout_json5(
    layout: &crate::layout::Layout,
    path: &std::path::Path,
) -> Result<()> {
    // Invert positions: key-id -> char.
    let mut id_to_char: std::collections::HashMap<String, char> =
        std::collections::HashMap::new();
    for (&ch, key) in &layout.positions {
        id_to_char.insert(key.id.clone(), ch);
    }

    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  name: \"{}\",\n", layout.name));
    out.push_str("  mappings: {\n");

    for n in 1..=30u32 {
        let id = format!("main_k{n}");
        if let Some(&ch) = id_to_char.get(&id) {
            let upper = ch.to_ascii_uppercase();
            out.push_str(&format!(
                "    {:<10} {{ char: [\"{}\", \"{}\"] }},\n",
                format!("{}:", id),
                ch,
                upper
            ));
        }
    }

    out.push_str("  },\n");
    out.push_str("}\n");

    std::fs::write(path, out)
        .with_context(|| format!("writing layout to {}", path.display()))?;
    Ok(())
}

/// Render `score` reports. When `json` is true, emits a JSON array
/// (one entry per corpus); otherwise prints the text report.
fn emit_score(reports: &[score::ScoreReport], json: bool) -> Result<()> {
    if json {
        let out = serde_json::to_string_pretty(reports)?;
        println!("{out}");
        return Ok(());
    }
    for (i, report) in reports.iter().enumerate() {
        if i > 0 {
            println!("\n{}\n", "────────────────────────────────────────");
        }
        report::print(report);
    }
    Ok(())
}

/// Render `compare` reports. When `json` is true, emits a JSON
/// array of `{ a, b }` objects; otherwise prints the side-by-side view.
fn emit_compare(pairs: &[(score::ScoreReport, score::ScoreReport)], json: bool) -> Result<()> {
    if json {
        #[derive(serde::Serialize)]
        struct Pair<'a> {
            a: &'a score::ScoreReport,
            b: &'a score::ScoreReport,
        }
        let entries: Vec<_> = pairs.iter().map(|(a, b)| Pair { a, b }).collect();
        let out = serde_json::to_string_pretty(&entries)?;
        println!("{out}");
        return Ok(());
    }
    for (i, (a, b)) in pairs.iter().enumerate() {
        if i > 0 {
            println!("\n{}\n", "────────────────────────────────────────");
        }
        report::print_compare(a, b);
    }
    Ok(())
}

fn load_config(path: Option<&std::path::Path>) -> Result<Config> {
    match path {
        Some(p) => Config::load_from(p),
        None => Config::load_default(),
    }
}

fn load_corpora(
    paths: &[PathBuf],
    blend_specs: &[String],
    config: &Config,
) -> Result<Vec<Corpus>> {
    if !blend_specs.is_empty() {
        return Ok(vec![load_blend(blend_specs)?]);
    }

    if paths.is_empty() {
        let crate_root = env!("CARGO_MANIFEST_DIR");
        let default = std::path::Path::new(crate_root).join(&config.corpus.default);
        return Ok(vec![Corpus::load(&default)?]);
    }

    paths
        .iter()
        .map(|p| Corpus::load(p).with_context(|| format!("loading corpus: {}", p.display())))
        .collect()
}

/// Parse blend specs of the form `path:weight` and build the blended
/// corpus. Weight may be integer or float.
fn load_blend(specs: &[String]) -> Result<Corpus> {
    let mut entries: Vec<(Corpus, f64)> = Vec::with_capacity(specs.len());
    for spec in specs {
        let (path_part, weight_part) = spec
            .rsplit_once(':')
            .ok_or_else(|| anyhow::anyhow!("blend spec must be `path:weight`: {spec}"))?;
        let weight: f64 = weight_part
            .parse()
            .with_context(|| format!("invalid weight in blend spec: {spec}"))?;
        let corpus = Corpus::load(std::path::Path::new(path_part))
            .with_context(|| format!("loading blend corpus: {path_part}"))?;
        entries.push((corpus, weight));
    }
    Corpus::blend(&entries)
}

fn load_keyboard(path: Option<&std::path::Path>) -> Result<Keyboard> {
    let default_path;
    let path = match path {
        Some(p) => p,
        None => {
            let crate_root = env!("CARGO_MANIFEST_DIR");
            default_path = std::path::Path::new(crate_root)
                .join("../keyboards/halcyon_elora_v2.json");
            &default_path
        }
    };
    Keyboard::load(path)
}
