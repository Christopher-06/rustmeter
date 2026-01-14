use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

pub fn do_analyze_command(
    tracing_folder: Option<PathBuf>,
    exit_flag: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    println!("Running analysis on tracing folder: {:?}", tracing_folder);
    Ok(())
}
