use anyhow::Context;
use espflash::connection::{Connection, ResetAfterOperation, ResetBeforeOperation};
use serialport::UsbPortInfo;

pub mod flashing;

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
    let com_port = serialport::COMPort::open(&serialport::new(port.port_name, 115200))
        .context("Cannot open ComPort")?;

    Ok(espflash::connection::Connection::new(
        com_port,
        usb_info,
        ResetAfterOperation::NoReset,
        ResetBeforeOperation::DefaultReset,
        115200,
    ))
}
