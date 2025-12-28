use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use anyhow::Context;
use crossbeam::select;

use crate::{
    cargo::cargo_child::CargoChildProcess, cli::CommandLineArgs, elf_file::FirmwareAddressMap,
    perfetto_backend::file_writer::spawn_perfetto_file_writer,
    tracing::tracing_instance::TracingInstance,
};
use crate::{
    flash_and_monitor::flash_and_monitor_chip, logs::defmt_decoding::DefmtDecoding,
    tracing::trace_data_decoder::TraceDataDecoder,
};

mod cargo;
mod cli;
mod elf_file;
mod espflash;
mod flash_and_monitor;
mod logs;
mod perfetto_backend;
mod probe_rs;
mod time;
mod tracing;

fn main() -> anyhow::Result<()> {
    // Set CTRL-C handler
    let exit_flag = Arc::new(AtomicBool::new(false));
    let r_exit_flag = exit_flag.clone();
    ctrlc::set_handler(move || {
        println!("CTRL-C received, exiting...");
        r_exit_flag.store(true, std::sync::atomic::Ordering::SeqCst);
    })?;

    // Parse command line arguments
    let args = CommandLineArgs::parse();

    // Start Cargo child process and gather elf path
    let mut cargo_child_process = CargoChildProcess::new_start_build(args.release, &args.project)?;
    let elf_path = cargo_child_process.wait_till_finished()?;
    let fw_addr_map = FirmwareAddressMap::new_from_elf_path(&elf_path)?;
    println!("Build Status: Success");
    println!("ELF Path: {:?}", elf_path);

    // flash and start monitoring
    let monitor = flash_and_monitor_chip(&args.chip, args.tool.clone(), &elf_path, &fw_addr_map)?;
    let defmt_bytes_recver = monitor.get_defmt_bytes_recver();
    let tracing_bytes_recver = monitor.get_tracing_bytes_recver();
    let monitor_error_recver = monitor.get_error_recver();

    // Create defmt/tracing decoding instance
    let mut tracing_decoding = TraceDataDecoder::new();
    let defmt_decoding = DefmtDecoding::new(&elf_path, defmt_bytes_recver, true)
        .context("Failed to create defmt decoder!")?;
    let defmt_logs_recver = defmt_decoding.get_defmt_logs_recver();

    // Create tracing instance
    let mut tracing_instance = TracingInstance::new(fw_addr_map);
    let trace_event_recver = tracing_instance.get_trace_event_receiver();

    // Create perfetto trace writer thread
    let perfetto_filename = Path::new(&args.project).join(format!(
        "rustmeter-perfetto-{}.json",
        if args.release { "release" } else { "debug" }
    ));
    let perfetto_file_writer_handle =
        spawn_perfetto_file_writer(perfetto_filename, trace_event_recver, exit_flag.clone());

    loop {
        // Check for exit flag
        if exit_flag.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }

        // check for perfetto file writer thread exit
        if perfetto_file_writer_handle.is_finished() {
            // normally this should not happen
            match perfetto_file_writer_handle.join() {
                Ok(result) => {
                    if let Err(e) = result {
                        println!("[Error] Perfetto file writer thread exited with error: {e}");
                    } else {
                        println!("[Info] Perfetto file writer thread exited normally.");
                    }
                    break;
                }
                Err(e) => {
                    println!("[Error] Perfetto file writer thread panicked: {e:?}");
                    break;
                }
            }
        }

        select! {
            // Receive next tracing bytes
            recv(tracing_bytes_recver) -> tracing_bytes_res => {
                match tracing_bytes_res {
                    Ok(tracing_bytes) => {
                        tracing_decoding.feed(&tracing_bytes);
                        let decoded_items = tracing_decoding.decode()?;
                        for item in decoded_items {
                            // println!("[Tracing] {:.6}s - {:?}", item.timestamp().as_secs_f64(), item.payload());
                            tracing_instance.feed(item, false);
                        }
                    }
                    Err(e) => {
                        println!("[Tracing RTT Error] {}", e);
                        break; // channel closed
                    }
                }
            },
            // Receive next defmt logs
            recv(defmt_logs_recver) -> defmt_log_res => {
                match defmt_log_res {
                    Ok(defmt_log) => {
                        tracing_instance.add_defmt_log(&defmt_log);
                    }
                    Err(e) => {
                        println!("[Defmt RTT Error] {}", e);
                        break; // channel closed
                    }
                }
            },
            // Receive next monitor error
            recv(monitor_error_recver) -> monitor_error_res => {
                match monitor_error_res {
                    Ok(monitor_error) => {
                        println!("[Monitor Error] {}", monitor_error);
                    }
                    Err(e) => {
                        println!("[Monitor Error Receiver Closed] {}", e);
                        break; // channel closed
                    }
                }                
            }
            default(Duration::from_millis(100)) => {
                // timeout ==> just continue to check exit_flag
                continue;
            }
        }
    }

    return Ok(());
}
