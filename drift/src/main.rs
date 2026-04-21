//! drift — flexion-aware keyboard layout scorer.
//!
//! Entry point. See `cli` for argument handling and `score` for the
//! scoring pipeline.

mod cli;
mod config;
mod corpus;
mod generate;
mod keyboard;
mod layout;
mod motion;
mod report;
mod score;

fn main() -> anyhow::Result<()> {
    cli::run()
}
