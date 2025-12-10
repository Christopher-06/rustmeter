use std::process::{Command, Stdio};

use crate::cargo::cargo_build::CargoBuildStatus;
use anyhow::Context;
use crossbeam::channel::{Receiver, Sender};

pub struct CargoChildProcess {
    child: std::process::Child,

    logs_recver: Receiver<String>,

    build_status_recver: Receiver<CargoBuildStatus>,
    current_build_status: CargoBuildStatus,
}

impl CargoChildProcess {
    pub fn kill(mut self) -> anyhow::Result<()> {
        self.child.kill().context("Tried to kill child process")
    }

    pub fn get_status_code(&mut self) -> anyhow::Result<Option<std::process::ExitStatus>> {
        self.child
            .try_wait()
            .context("Failed to try waiting on child process to gather exit status")
    }

    /// Synchronously waits for the build process to finish.
    pub fn wait_build_finish(&mut self) -> anyhow::Result<CargoBuildStatus> {
        // Wait for updates
        while let Ok(val) = self.build_status_recver.recv() {
            self.current_build_status = val;

            if self.current_build_status.has_finished() {
                return Ok(self.current_build_status.clone());
            }
        }

        // Sender has been closed ==> unexpected error
        Err(anyhow::anyhow!("Build status channel closed unexpectedly"))
    }

    pub fn get_logs_receiver(&self) -> Receiver<String> {
        self.logs_recver.clone()
    }

    pub fn new_start_run(release: bool, project_dir: &str) -> anyhow::Result<Self> {
        let (build_status_sender, build_status_recver) = crossbeam::channel::unbounded();
        let (logs_sender, logs_recver) = crossbeam::channel::unbounded();

        // Create Command
        let mut cmd = Command::new("cargo");
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit()); // directly inherit stderr to main process
        cmd.current_dir(project_dir);

        // Add arguments
        cmd.arg("run");
        cmd.arg("--message-format")
            .arg("json-diagnostic-rendered-ansi"); // for easier parsing of build output
        if release {
            cmd.arg("--release");
        }

        // Spawn process and take stdout
        let mut child = cmd.spawn().context("Failed to spawn cargo process")?;
        let stdout = child
            .stdout
            .take()
            .context("Failed to take stdout of cargo process")?;
        let _ = read_to_channel_threaded(stdout, build_status_sender, logs_sender);

        Ok(CargoChildProcess {
            child,
            logs_recver,
            build_status_recver,
            current_build_status: CargoBuildStatus::Started,
        })
    }
}

impl Drop for CargoChildProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// Reads from the given reader and sends the output to the provided channel sender.
fn read_to_channel_threaded<R: std::io::Read + Send + 'static>(
    mut reader: R,
    build_status_sender: Sender<CargoBuildStatus>,
    logs_sender: Sender<String>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut byte_buffer = [0; 1024];
        let mut string_buffer = String::new();

        let mut last_build_status = CargoBuildStatus::Started;

        loop {
            // Read next bytes or exit
            match reader.read(&mut byte_buffer) {
                Ok(n) => {
                    if n > 0 {
                        // Convert bytes to string and append to buffer
                        let chunk = String::from_utf8_lossy(&byte_buffer[..n]);
                        string_buffer.push_str(&chunk);
                    } else {
                        break; // EOF (process ended)
                    }
                }
                Err(e) => {
                    eprintln!("Error reading cargo run output: {e}");
                    break;
                }
            }

            // Process complete lines
            while let Some(pos) = string_buffer.find('\n') {
                let line = string_buffer.drain(..=pos).collect::<String>();
                if !last_build_status.has_finished() {
                    // Parse Build line to CargoBuildStatus
                    last_build_status =
                        CargoBuildStatus::update_from_build_line(last_build_status, &line);
                    let ch_closed = build_status_sender.send(last_build_status.clone()).is_err();

                    if ch_closed || last_build_status.has_failed() {
                        return; // Stop processing if receiver is closed or build failed
                    }
                } else {
                    // Log line
                    if logs_sender.send(line).is_err() {
                        return; // Stop processing if receiver is closed
                    }
                }
            }
        }
    })
}
