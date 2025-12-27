use std::{path::PathBuf, time::Duration};

use anyhow::Context;
use espflash::{
    connection::Connection,
    flasher::{FlashData, FlashSettings},
    image_format::{ImageFormat, idf::IdfBootloaderFormat},
    target::ProgressCallbacks,
};
use indicatif::{ProgressBar, ProgressStyle};

pub fn flash_esp(conn: Connection, chip: &str, elf_path: &PathBuf) -> anyhow::Result<Connection> {
    // connect flasher
    let mut flasher = espflash::flasher::Flasher::connect(
        conn,
        true,
        true,
        false,
        Some(chip.try_into().context("Invalid chip name")?),
        Some(921600),
    )
    .context("Failed to connect Flasher")?;
    let chip = flasher.chip();

    let device_info = flasher.device_info().context("Failed to get device info")?;

    let flash_settings = FlashSettings::default();
    let xtal_freq = chip
        .xtal_frequency(flasher.connection())
        .context("Cannot get XtalFreq")?;
    let (_, min_chip_rev) = device_info
        .revision
        .context("Cannot get Min Chip Revision")?;
    let flash_data = FlashData::new(flash_settings, min_chip_rev as u16, None, chip, xtal_freq);

    let elf_data = std::fs::read(elf_path).context("Failed to read ELF file")?;
    let idf_bootloader = IdfBootloaderFormat::new(&elf_data, &flash_data, None, None, None, None)
        .context("Can't create IdfBootloaderFormat")?;

    flasher
        .load_image_to_flash(
            &mut FlashProgress::new(),
            ImageFormat::EspIdf(idf_bootloader),
        )
        .context("error flashing elf file")?;

    // Reset device after flashing
    flasher
        .connection()
        .reset()
        .context("Failed to reset after flashing")?;

    Ok(flasher.into_connection())
}

struct FlashProgress {
    progress_bar: ProgressBar,
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
            progress_bar: ProgressBar::new(0),
            style,
        }
    }
}

impl ProgressCallbacks for FlashProgress {
    fn init(&mut self, addr: u32, total: usize) {
        self.progress_bar = ProgressBar::new(total as u64);
        self.progress_bar
            .set_message(format!("Flashing {:X}...", addr));
        self.progress_bar.set_style(self.style.clone());
    }

    fn update(&mut self, current: usize) {
        self.progress_bar.set_position(current as u64);
    }

    fn finish(&mut self, skipped: bool) {
        if skipped {
            self.progress_bar.finish_with_message("Skipped");
        } else {
            self.progress_bar
                .finish_with_message("✅ Flashing completed");
        }
    }

    fn verifying(&mut self) {
        self.progress_bar.finish();

        // Create spinner
        self.progress_bar = ProgressBar::new_spinner();
        self.progress_bar.set_message("Verifying...");
        self.progress_bar
            .enable_steady_tick(Duration::from_millis(100));
    }
}

impl Drop for FlashProgress {
    fn drop(&mut self) {
        self.progress_bar
            .finish_with_message("Flashing and Verifying done");
    }
}
