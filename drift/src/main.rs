//! drift — flexion-aware keyboard layout scorer.
//!
//! Entry point. See `cli` for argument handling and `score` for the
//! scoring pipeline.

mod cli;
mod config;
mod corpus;
mod delta;
mod generate;
mod keyboard;
mod layout;
mod motion;
mod report;
mod score;
mod trigram;

fn main() -> anyhow::Result<()> {
    cli::run()
}
