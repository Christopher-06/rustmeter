use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use crate::{
    cargo::{cargo_child::CargoChildProcess, elf_file::FirmwareAddressMap},
    cli::RunArgs,
    commands::flash_and_monitor::flash_and_monitor_chip,
    tracing::sink::TracingSink,
};

/// Execute the 'run' command: build (if needed), flash, and monitor tracing data. Returns the
/// tracing folder path
pub fn do_run_command(mut args: RunArgs, exit_flag: Arc<AtomicBool>) -> anyhow::Result<PathBuf> {
    // Get elf path
    let elf_path = match &args.executable {
        Some(path) => {
            // use provided elf path

            // check for release build (this info is not given when used in cargo runner)
            if path.to_string_lossy().contains("/release/") {
                args.release = true;
            }

            path.clone()
        }
        None => {
            // build project and get elf path
            let mut cargo_child_process =
                CargoChildProcess::new_start_build(args.release, &args.project)?;
            let elf_path = cargo_child_process.wait_till_finished()?;
            println!("Build Status: Success");
            elf_path
        }
    };

    // Create firmware address map from elf file
    let fw_addr_map = FirmwareAddressMap::new_from_elf_path(&elf_path)?;

    // flash and start monitoring
    let monitor = flash_and_monitor_chip(&args.chip, args.tool.clone(), &elf_path, &fw_addr_map)?;
    let tracing_bytes_recver = monitor.get_tracing_bytes_recver();
    let req_sender = monitor.get_request_sender();

    // Create new Tracing Folder aside the elf file and remove existing one
    let tracing_folder = elf_path.parent().unwrap().join("tracing");
    if tracing_folder.exists() {
        std::fs::remove_dir_all(&tracing_folder)?;
    }
    std::fs::create_dir(&tracing_folder)?;

    // Record tracing data till exit flag is set
    let mut tracing_sink = TracingSink::new(
        tracing_folder.clone(),
        &elf_path,
        tracing_bytes_recver,
        req_sender,
        &args,
    )?;
    tracing_sink.sink_bytes(exit_flag.clone())?;
    tracing_sink.finalize()?;

    Ok(tracing_folder)
}
