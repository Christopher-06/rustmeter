use std::{
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

use crate::cargo::cargo_build::CargoBuildMessage;
use anyhow::{Context, bail};

/// Represents a child process running a Cargo Build command.
pub struct CargoChildProcess {
    /// Receiver for build status updates
    child: std::process::Child,

    /// Path to the built executable (if available)
    elf_path: Arc<Mutex<Option<PathBuf>>>,
}

impl CargoChildProcess {
    /// Synchronously waits for the process to finish and returns the path to the built executable.
    pub fn wait_till_finished(&mut self) -> anyhow::Result<PathBuf> {
        // wait till finished
        let exit_status = self
            .child
            .wait()
            .context("Failed to wait on child process")?;
        if !exit_status.success() {
            bail!("Child process exited with non-zero status: {}", exit_status);
        }

        // get executable path
        let elf_path = match self.elf_path.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => bail!("Failed to lock elf_path mutex"),
        };

        match elf_path {
            Some(path) => Ok(path),
            None => bail!("Executable path not found after process finished"),
        }
    }

    /// Starts a new Cargo build process in the specified project directory with the given release flag.
    pub fn new_start_build(release: bool, project_dir: &str) -> anyhow::Result<Self> {
        // Create Command
        let mut cmd = Command::new("cargo");
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit()); // directly inherit stderr to main process
        cmd.current_dir(project_dir);

        // Add arguments
        cmd.arg("build");
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

        let elf_path = Arc::new(Mutex::new(None));
        let _ = read_to_channel_threaded(stdout, elf_path.clone());

        Ok(CargoChildProcess { child, elf_path })
    }
}

/// Reads from the given reader and sends the output to the provided channel sender.
fn read_to_channel_threaded<R: std::io::Read + Send + 'static>(
    mut reader: R,
    elf_path: Arc<Mutex<Option<PathBuf>>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut byte_buffer = [0; 1024];
        let mut string_buffer = String::new();

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
                let parse_res = CargoBuildMessage::from_build_line(line.trim());

                match parse_res {
                    Ok(message) => {
                        // Check for elf path
                        if let Some(elf_path_buf) = message.get_elf_path() {
                            if let Ok(mut guard) = elf_path.lock() {
                                // store elf path
                                *guard = Some(elf_path_buf);
                            }
                        }
                    }
                    Err(_) => {
                        // Just text output, print it
                        if !line.trim().starts_with("{") {
                            print!("{line}");
                        }
                    }
                }
            }
        }
    })
}
