use embassy_futures::select::select;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, pipe::Pipe, signal::Signal};
use embassy_time::{Duration, Timer};
use embedded_io_async::Write;
use esp_hal::Async;
use esp_hal::time::Rate;
use esp_hal::uart::Uart;
use rustmeter_beacon_core::protocol::{EventPayload, Request, TypeDefinitionPayload};
use rustmeter_beacon_core::tracing::write_tracing_event;

use crate::TICK_DIVIDER;
use crate::espressif::esp_defmt_pipe;
use crate::espressif::tracing_esp;
use crate::ringbuffer::SimpleRingBuffer;

pub enum PrinterRoute {
    Uart(Uart<'static, Async>),
    #[cfg(any(feature = "esp32c3", feature = "esp32c6", feature = "esp32h2", feature = "esp32s3"))]
    SerialJtag(esp_hal::usb_serial_jtag::UsbSerialJtag<'static, Async>),
}

impl PrinterRoute {
    /// Write all data asynchronously to selected output
    pub async fn write_all(&mut self, data: &[u8]) -> Result<(), ()> {
        match self {
            PrinterRoute::Uart(serial) => match serial.write_all(&data).await {
                Ok(_) => Ok(()),
                Err(_) => Err(()),
            },
            #[cfg(any(feature = "esp32c3", feature = "esp32c6", feature = "esp32h2", feature = "esp32s3"))]
            PrinterRoute::SerialJtag(jtag) => match jtag.write_all(&data).await {
                Ok(_) => Ok(()),
                Err(_) => Err(()),
            },
        }
    }

    /// Read available bytes from selected input
    pub fn read_bytes(&mut self, buf: &mut [u8]) -> usize {
        match self {
            PrinterRoute::Uart(serial) => {
                if serial.read_ready() {
                    // read is blocking, so only call if data is available
                    serial.read(buf).unwrap_or(0)
                } else {
                    0
                }
            }
            #[cfg(any(feature = "esp32c3", feature = "esp32c6", feature = "esp32h2", feature = "esp32s3"))]
            PrinterRoute::SerialJtag(jtag) => {
                let mut n = 0;
                loop {
                    if n >= buf.len() {
                        break n;
                    }

                    match jtag.read_byte() {
                        Ok(b) => {
                            buf[n] = b;
                            n += 1;
                        }
                        Err(_) => break n,
                    }
                }
            }
        }
    }
}

/// Task that prints internal tracing and logging data to output
#[embassy_executor::task]
pub async fn connector(mut out_route: PrinterRoute, cpu_freq: Rate) {
    // Get pipes
    let (trace_buffer, trace_data_signal) = tracing_esp::get_trace_pipe_and_signal();
    #[cfg(feature = "defmt")]
    let (defmt_data_pipe, defmt_data_signal) = esp_defmt_pipe::get_defmt_pipe_and_signal();

    // Working buffer buffer
    let mut buffer = [0u8; 128]; // 128 byte buffer is ESP UART FIFO size and 64 bytes is USB Serial-JTAG FIFO size
    buffer[0] = 0xFF; // Start byte

    // Receive buffer
    let mut recvd_buffer = SimpleRingBuffer::<128>::new();

    loop {
        // Wait for any new datadata or timeout
        // let _ = select(
        //     trace_data_signal.wait(),
        //     select(
        //         defmt_data_signal.wait(),
        //         Timer::after(Duration::from_millis(100)),
        //     ),
        // )
        // .await;

        trace_data_signal.wait().await;

        // Process tracing data per core
        for (core_id, buf) in trace_buffer.iter().enumerate() {
            for _ in 0..30 {
                let len = {
                    let buf0 = unsafe { &mut *buf.get() };
                    buf0.pop_slice(&mut buffer[3..127])
                };

                if len > 0 {
                    trace_data_signal.reset();

                    // Create Header
                    buffer[1] = 0xF0 + (core_id as u8);
                    buffer[2] = len as u8; // length byte

                    // Calculate xor checksum and send
                    buffer[len + 3] = calculate_checksum(&buffer[1..(3 + len)]);
                    let _ = out_route.write_all(&buffer[0..3 + len + 1]).await;
                } else {
                    break;
                }
            }
        }

        // Process defmt data
        #[cfg(feature = "defmt")]
        read_and_write_pipe(
            defmt_data_pipe,
            defmt_data_signal,
            &mut buffer,
            0x02,
            &mut out_route,
        )
        .await;

        // try to read requests
        let n = out_route.read_bytes(&mut buffer[1..128]);
        if n > 0 {
            // Push to ringbuffer or drain old data
            let to_drain = n - recvd_buffer.free();
            if to_drain > 0 {
                recvd_buffer.drain(to_drain);
            }
            let _ = recvd_buffer.push_slice(&buffer[1..1 + n]);

            // Try to decode some bytes
            match Request::from_bytes(recvd_buffer.iter()) {
                Some((request, n)) => {
                    match request {
                        Request::GetGlobalClockDefinition => {
                            // Send clock definition response via TypeDefinition
                            write_tracing_event(EventPayload::TypeDefinition(
                                TypeDefinitionPayload::GlobalClockConfiguration {
                                    cpu_frequency_hz: cpu_freq.as_hz(),
                                    tick_divider: TICK_DIVIDER as u16,
                                },
                            ));
                        }
                        Request::GetCoreClockReference { core_id } => {
                            // Reset core clock referenced
                            let core_id = core_id as usize;
                            use rustmeter_beacon_core::time_delta::CORE_CLOCK_REFERENCED;
                            if core_id < CORE_CLOCK_REFERENCED.len() {
                                CORE_CLOCK_REFERENCED[core_id]
                                    .store(false, portable_atomic::Ordering::Relaxed);
                            }
                        }
                    }

                    // drain used bytes
                    recvd_buffer.drain(n);
                }
                None => {
                    // Not enough data yet
                }
            }
        }
    }
}

/// Read all available data from the pipe and write it to UART with header and checksum
async fn read_and_write_pipe<'a, const N: usize>(
    pipe: &Pipe<CriticalSectionRawMutex, N>,
    new_data_signal: &Signal<CriticalSectionRawMutex, ()>,
    buffer: &mut [u8; 128],
    type_id: u8,
    tx: &mut PrinterRoute,
) {
    while let Ok(n_bytes) = pipe.try_read(&mut buffer[3..127]) {
        new_data_signal.reset();

        // Create Header
        buffer[1] = type_id;
        buffer[2] = n_bytes as u8; // length byte

        // Calculate xor checksum and send
        buffer[n_bytes + 3] = calculate_checksum(&buffer[1..(3 + n_bytes)]);
        let _ = tx.write_all(&buffer[0..3 + n_bytes + 1]).await;
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
