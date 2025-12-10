use std::{
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use crossbeam::select;

use crate::{
    cargo::cargo_child::CargoChildProcess, cli::CommandLineArgs, elf_file::FirmwareAddressMap,
    perfetto_backend::file_writer::spawn_perfetto_file_writer,
    tracing::tracing_instance::TracingInstance,
};

mod cargo;
mod cli;
mod elf_file;
mod perfetto_backend;
mod time;
mod tracing;

fn main() -> anyhow::Result<()> {
    // Set CTRL-C handler
    let exit_flag = Arc::new(AtomicBool::new(false));
    let r_exit_flag = exit_flag.clone();
    ctrlc::set_handler(move || {
        r_exit_flag.store(true, std::sync::atomic::Ordering::SeqCst);
    })?;

    // Parse command line arguments
    let args = CommandLineArgs::parse();

    // Start Cargo child process and wait for build to finish
    let mut cargo_child_process = CargoChildProcess::new_start_run(args.release, &args.project)?;
    let build_status = cargo_child_process.wait_build_finish()?;

    // Check build status
    if build_status.has_failed() {
        // cargo build failed ==> it printed error messages already
        return Err(anyhow::anyhow!(
            "Cargo build failed. Cannot start tracing session."
        ));
    }

    // Get executable path
    let elf_path = build_status
        .try_get_executable()
        .clone()
        .ok_or(anyhow::anyhow!(
            "Cannot get executable path from build status"
        ))?;
    let elf_path = Path::new(&elf_path);
    let firmware_addr_map = FirmwareAddressMap::new_from_elf_path(elf_path)?;

    // filter log events and print everything else to stdout
    let raw_logs_recver = cargo_child_process.get_logs_receiver();
    let (log_line_sender, log_line_recver) = crossbeam::channel::unbounded();
    let (log_event_sender, log_event_recver) = crossbeam::channel::unbounded();
    std::thread::spawn(move || {
        while let Ok(log) = raw_logs_recver.recv() {
            // try to parse log line as LogEvent or just print it
            if let Ok(log_line) = tracing::log_line::LogLine::from_str(&log) {
                // Check if it is a LogEvent
                if let Ok(log_event) = tracing::log_event::LogEvent::from_log_line(&log_line) {
                    // successfully parsed LogEvent ==> send it as log event
                    if log_event_sender.send(log_event).is_err() {
                        break; // channel closed
                    }

                    continue;
                } else {
                    // send log line as well for raw logging
                    println!("{log_line}");

                    // is log line ==> send log line
                    if log_line_sender.send(log_line).is_err() {
                        break; // channel closed
                    }
                }
            } else {
                // cannot parse it correctly ==> just print the raw log
                print!("{log}");
            }
        }

        // error returned because channel closed
    });

    // Create tracing instance and start processing log events
    let mut tracing_instance = TracingInstance::new(firmware_addr_map);
    let trace_event_recver = tracing_instance.get_trace_event_receiver();
    std::thread::spawn(move || {
        loop {
            // receive next log-event or log-line
            select! {
                recv(log_line_recver) -> log_line_res => {
                    // got log line
                    match log_line_res {
                        Ok(log_line) => {
                            tracing_instance.add_log_line(&log_line);
                        }
                        Err(_) => break, // channel closed
                    }
                },
                recv(log_event_recver) -> log_event_res => {
                    // got log event
                    match log_event_res {
                        Ok(log_event) => {
                            tracing_instance.update(&log_event);
                        }
                        Err(_) => break, // channel closed
                    }
                },
            }
        }
    });

    // Create Perfetto trace writer and start writing trace events from trace_event_recver
    let perfetto_filename = Path::new(&args.project).join(format!(
        "rustmeter-perfetto-{}.json",
        if args.release { "release" } else { "debug" }
    ));
    let perfetto_file_writer_handle =
        spawn_perfetto_file_writer(perfetto_filename, trace_event_recver, exit_flag.clone());

    // Main loop
    while !exit_flag.load(std::sync::atomic::Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));

        // Check if cargo child process has exited
        if let Some(status_code) = cargo_child_process.get_status_code()? {
            return Err(anyhow::anyhow!(
                "Cargo process exited with status: {status_code}"
            ));
        }

        // Check if perfetto file writer thread has exited with error
        if perfetto_file_writer_handle.is_finished() {
            // normally this should not happen
            match perfetto_file_writer_handle.join() {
                Ok(result) => {
                    if let Err(e) = result {
                        return Err(anyhow::anyhow!(
                            "Perfetto file writer thread exited with error: {e}"
                        ));
                    } else {
                        return Ok(()); // normal exit
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!(
                        "Perfetto file writer thread panicked: {e:?}"
                    ));
                }
            }
        }
    }

    // Clean up
    cargo_child_process.kill()?;
    perfetto_file_writer_handle.join().unwrap()?;

    Ok(())
}
