//! The `drift` CLI binary. Thin wrapper over [`drift_cli::dispatch`].

fn main() -> anyhow::Result<()> {
    drift_cli::dispatch()
}
