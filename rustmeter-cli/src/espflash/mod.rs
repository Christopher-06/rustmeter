use anyhow::Context;
use espflash::connection::{Connection, ResetAfterOperation, ResetBeforeOperation};
use serialport::UsbPortInfo;

pub mod flashing;
mod framing;
mod serial_decoder;

pub mod serial_listener;

pub fn get_espflash_connection() -> anyhow::Result<Connection> {
    // get current port
    let port = serialport::available_ports()?
        .into_iter()
        .next()
        .context("No Port found")?;
    let usb_info = match &port.port_type {
        serialport::SerialPortType::UsbPort(info) => UsbPortInfo {
            vid: info.vid,
            pid: info.pid,
            serial_number: info.serial_number.clone(),
            manufacturer: info.manufacturer.clone(),
            product: info.product.clone(),
            interface: info.interface,
        },
        _ => anyhow::bail!("Port is not a USB port"),
    };

    // open serial port
    let serial = espflash::connection::Port::open(&serialport::new(port.port_name, 115200))
        .context("Cannot open serial port")?;

    Ok(espflash::connection::Connection::new(
        serial,
        usb_info,
        ResetAfterOperation::NoReset,
        ResetBeforeOperation::DefaultReset,
        115200,
    ))
}

#[derive(Debug)]
pub enum SerialFrameError {
    ChecksumMismatch,
    UnknownFrameMode(u8),
    SequenceIdMismatch {
        expected: u8,
        received: u8,
        core_id: u8,
    },
    DuplicateData,
}

impl From<&SerialFrameError> for anyhow::Error {
    fn from(val: &SerialFrameError) -> Self {
        match val {
            SerialFrameError::ChecksumMismatch => anyhow::anyhow!("Checksum mismatch"),
            SerialFrameError::SequenceIdMismatch {
                expected,
                received,
                core_id,
            } => anyhow::anyhow!(
                "Sequence ID mismatch on core {}: expected {}, received {}",
                core_id,
                expected,
                received
            ),
            SerialFrameError::DuplicateData => anyhow::anyhow!("Duplicate frame data received"),
            SerialFrameError::UnknownFrameMode(mode) => {
                anyhow::anyhow!("Unknown frame mode: {}", mode)
            }
        }
    }
}
