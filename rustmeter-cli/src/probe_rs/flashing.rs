use std::{path::PathBuf, time::Duration};

use probe_rs::flashing::{self, DownloadOptions, ElfOptions, FlashProgress};

use crate::probe_rs::{
    atomic_session::AtomicSession,
    flash_progress::{progress_handler, reset_progress},
};

fn define_download_options<'a>() -> DownloadOptions<'a> {
    reset_progress();

    let mut download_options = DownloadOptions::default();
    download_options.verify = true;
    download_options.do_chip_erase = true;
    download_options.progress = FlashProgress::new(Box::new(progress_handler));

    download_options
}

/// Flash the given ELF file to the target and start the controller core.
pub fn flash_and_start_controller<'a>(
    session: &AtomicSession,
    elf_path: &PathBuf,
) -> anyhow::Result<()> {
    let mut session = session.lock();

    // Start flashing the ELF file
    probe_rs::flashing::download_file_with_options(
        &mut session,
        elf_path,
        flashing::Format::Elf(ElfOptions::default()),
        define_download_options(),
    )?;

    // Reset and run the core
    let mut core = session.core(0)?;
    core.reset_and_halt(Duration::from_millis(100))?;
    core.run()?;
    println!("Flashing completed successfully.");

    Ok(())
}
