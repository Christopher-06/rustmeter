use embassy_futures::select::select;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, pipe::Pipe, signal::Signal};
use embassy_time::{Duration, Timer};
use esp_hal::{Async, uart::UartTx};

use crate::espressif::esp_defmt_pipe;
use crate::espressif::espressif_config;
use crate::espressif::tracing_esp;

/// Task that prints internal tracing and logging data to output
#[embassy_executor::task]
pub async fn trace_data_printing(config: espressif_config::Config<'static>) {
    // Build Uart
    let uart = esp_hal::uart::Uart::new(
        config.uart_p,
        esp_hal::uart::Config::default().with_baudrate(config.baudrate),
    )
    .unwrap()
    .with_tx(config.tx_pin)
    .with_rx(config.rx_pin)
    .into_async();

    let (_rx, mut tx) = uart.split();

    // Get pipes
    let (trace_data_pipe, trace_data_signal) = tracing_esp::get_trace_pipe_and_signal();
    #[cfg(feature = "defmt")]
    let (defmt_data_pipe, defmt_data_signal) = esp_defmt_pipe::get_defmt_pipe_and_signal();

    let mut buffer = [0u8; 128]; // 128 byte buffer is ESP UART FIFO size
    buffer[0] = 0xFF; // Start byte
    loop {
        // Wait for any new datadata or timeout
        let _ = select(
            trace_data_signal.wait(),
            select(
                defmt_data_signal.wait(),
                Timer::after(Duration::from_millis(100)),
            ),
        )
        .await;

        // Process tracing data
        read_and_write_pipe(
            trace_data_pipe,
            trace_data_signal,
            &mut buffer,
            0x01,
            &mut tx,
        )
        .await;

        // Process defmt data
        #[cfg(feature = "defmt")]
        read_and_write_pipe(
            defmt_data_pipe,
            defmt_data_signal,
            &mut buffer,
            0x02,
            &mut tx,
        )
        .await;
    }
}

/// Read all available data from the pipe and write it to UART with header and checksum
async fn read_and_write_pipe<'a, const N: usize>(
    pipe: &Pipe<CriticalSectionRawMutex, N>,
    new_data_signal: &Signal<CriticalSectionRawMutex, ()>,
    buffer: &mut [u8; 128],
    type_id: u8,
    tx: &mut UartTx<'a, Async>,
) {
    while let Ok(n_bytes) = pipe.try_read(&mut buffer[3..127]) {
        new_data_signal.reset();

        // Create Header
        buffer[1] = type_id;
        buffer[2] = n_bytes as u8; // length byte

        // Calculate xor checksum and send
        buffer[n_bytes + 3] = calculate_checksum(&buffer[1..(3 + n_bytes)]);
        write_all(tx, &buffer[0..3 + n_bytes + 1]).await;
    }
}

/// Calculate XOR checksum
fn calculate_checksum(data: &[u8]) -> u8 {
    let mut checksum: u8 = 0;
    for &b in data {
        checksum ^= b;
    }
    checksum
}

/// Simple async write all function for UART to retry until all bytes are written
async fn write_all<'a>(tx: &mut UartTx<'a, Async>, data: &[u8]) {
    let mut bytes_written = 0;
    while bytes_written < data.len() {
        match tx.write_async(&data[bytes_written..]).await {
            Ok(n) => bytes_written += n,
            Err(e) => {
                #[cfg(feature = "defmt")]
                defmt::error!("UART write error: {:?}", e);
                break;
            }
        }
    }
}
