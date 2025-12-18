use anyhow::Context;
use probe_rs::probe::{Probe, list::Lister};

mod flash_progress;
pub mod flashing;

/// Connects to the first available probe.
pub fn connect_to_first_probe() -> anyhow::Result<Probe> {
    let lister = Lister::new();
    let probe = lister
        .list_all()
        .into_iter()
        .next()
        .context("No probe found")?;

    probe.open().context("Failed to open probe")
}
