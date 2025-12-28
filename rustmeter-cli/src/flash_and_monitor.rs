use std::{path::PathBuf, time::Duration};

use crate::{
    cli::FlashingTool,
    elf_file::FirmwareAddressMap,
    espflash::{flashing::flash_esp, get_espflash_connection, serial_listener},
    probe_rs::{
        connect_to_first_probe, flashing::flash_and_start_controller, rtt_listener::RttListener,
    },
};

/// Simple Trait for Chip Monitoring Tool (e.g., probe-rs RTT or espflash Serial)
pub trait ChipMonitoringTool {
    fn get_defmt_bytes_recver(&self) -> crossbeam::channel::Receiver<Box<[u8]>>;
    fn get_tracing_bytes_recver(&self) -> crossbeam::channel::Receiver<Box<[u8]>>;
    fn get_error_recver(&self) -> crossbeam::channel::Receiver<anyhow::Error>;
}

pub fn flash_and_monitor_chip(
    chip: &str,
    tool: FlashingTool,
    elf_path: &PathBuf,
    fw_addr_map: &FirmwareAddressMap,
) -> anyhow::Result<Box<dyn ChipMonitoringTool>> {
    match tool {
        FlashingTool::Espflash => {
            // establish espflash connection and flash
            let espflash_conn = flash_esp(get_espflash_connection()?, chip, elf_path)?;

            // Get Serial listener
            let serial_listener = serial_listener::SerialListener::new(espflash_conn)?;
            Ok(Box::new(serial_listener))
        }
        FlashingTool::ProbeRs => {
            // establish probe-rs connection and flash
            let probe = connect_to_first_probe()?;
            let session = probe.attach(chip, Default::default())?.into();
            flash_and_start_controller(&session, elf_path)?;

            // Get Rtt listener (sleep a bit to allow target to initialize RTT)
            std::thread::sleep(Duration::from_millis(100));
            let rtt_address = fw_addr_map.get_rtt_symbol_address();
            let rtt_listener = RttListener::new(session.clone(), rtt_address)?;
            Ok(Box::new(rtt_listener))
        }
        FlashingTool::Auto => {
            // Choose default tool based on chip name
            if chip.to_lowercase().starts_with("esp32") {
                flash_and_monitor_chip(chip, FlashingTool::Espflash, elf_path, fw_addr_map)
            } else {
                flash_and_monitor_chip(chip, FlashingTool::ProbeRs, elf_path, fw_addr_map)
            }
        }
    }
}
