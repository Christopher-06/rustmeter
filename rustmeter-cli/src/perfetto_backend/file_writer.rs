use std::{
    fs::File,
    io::Write,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

use anyhow::Context;
use crossbeam::channel::Receiver;

use crate::perfetto_backend::trace_event::TracingEvent;

pub fn spawn_perfetto_file_writer(
    perfetto_filename: PathBuf,
    trace_event_recver: Receiver<TracingEvent>,
    exit_flag: Arc<AtomicBool>,
) -> JoinHandle<anyhow::Result<()>> {
    std::thread::spawn(move || {
        // Create file
        let mut file = File::options()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&perfetto_filename)
            .context("Failed to open perfetto trace file")?;
        file.write("{\"traceEvents\": [".as_bytes())
            .context("Failed to write to perfetto trace file")?;

        let mut first_event = true;
        while !exit_flag.load(Ordering::SeqCst) {
            match trace_event_recver.recv() {
                Ok(trace_event) => {
                    // write comma if not first event
                    if !first_event {
                        file.write_all(b",\n")
                            .context("Failed to add comma seperator")?;
                    } else {
                        first_event = false;
                    }

                    // write trace event as json
                    let json_str = "\t".to_string()
                        + &trace_event
                            .to_json()
                            .context("Failed to jsonify trace event")?;
                    file.write_all(json_str.as_bytes())
                        .context("Failed to write trace event to perfetto file")?;
                }
                Err(_) => break, // channel closed
            }
        }

        // finalise file and exit
        file.write_all(b"\n]}\n")
            .context("Failed to finalise perfetto trace file")?;
        return Ok(());
    })
}
