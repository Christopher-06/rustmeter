use std::{collections::HashMap, sync::Mutex, time::Duration};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use probe_rs::flashing::{ProgressEvent, ProgressOperation};

/// Stringify a value via format debug
macro_rules! to_string {
    ($val:expr) => {
        format!("{:?}", $val).to_string()
    };
}

// Progress Event Order: All AddProgressBar -> FlashLayoutReady -> (Started -> Progress -> Finished) per operation

struct FlashProgress {
    progress_bars: HashMap<String, ProgressBar>,
    progress_container: MultiProgress,
    style: ProgressStyle,
}

impl FlashProgress {
    pub fn new() -> Self {
        let style = ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap()
        .progress_chars("#>-");

        Self {
            progress_container: MultiProgress::new(),
            progress_bars: HashMap::new(),
            style,
        }
    }

    /// Add a new operation with optional total size
    pub fn add_operation(&mut self, operation: ProgressOperation, total: Option<u64>) {
        let pg = ProgressBar::new(total.unwrap_or(1));
        let pg = self.progress_container.add(pg);

        pg.set_style(self.style.clone());
        pg.set_message(format!("   {:?}", operation));

        self.progress_bars.insert(to_string!(operation), pg);
    }

    /// Start an operation
    pub fn start_operation(&mut self, operation: ProgressOperation) {
        if let Some(pb) = self.progress_bars.get(&to_string!(operation)) {
            pb.enable_steady_tick(Duration::from_millis(100));
        }
    }

    pub fn update_operation(&mut self, operation: ProgressOperation, size: u64) {
        if let Some(pb) = self.progress_bars.get(&to_string!(operation)) {
            pb.inc(size);
        }
    }

    /// Operation finished
    pub fn finished_operation(&mut self, operation: ProgressOperation, success: bool) {
        // Finish progress bar
        if let Some(pb) = self.progress_bars.get(&to_string!(operation)) {
            let icon = if success { "✅" } else { "❌" };
            pb.finish_with_message(format!("{icon} {:?}", operation));
        }
    }
}

static FLASH_PROGRESS: Mutex<Option<FlashProgress>> = Mutex::new(None);

pub fn progress_handler(progress: ProgressEvent) {
    // Lock and get flash progress
    let mut flash_progress = FLASH_PROGRESS.lock().unwrap();
    let flash_progress = flash_progress.get_or_insert_with(FlashProgress::new);

    // Handle progress event
    match progress {
        ProgressEvent::AddProgressBar { operation, total } => {
            flash_progress.add_operation(operation, total);
        }
        ProgressEvent::Started(operation) => {
            flash_progress.start_operation(operation);
        }
        ProgressEvent::Progress {
            operation, size, ..
        } => {
            flash_progress.update_operation(operation, size);
        }
        ProgressEvent::Finished(operation) => {
            flash_progress.finished_operation(operation, true);
        }
        ProgressEvent::FlashLayoutReady { .. } => { /* Ignore */ }
        ProgressEvent::DiagnosticMessage { message } => {
            println!("Diagnostic: {message}");
        }
        ProgressEvent::Failed(operation) => {
            flash_progress.finished_operation(operation, false);
        }
    }
}

/// Resets the flash progress state
pub fn reset_progress() {
    let mut flash_progress = FLASH_PROGRESS.lock().unwrap();
    *flash_progress = None;
}
