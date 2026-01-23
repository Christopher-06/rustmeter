use crossbeam::channel::{Receiver, Sender};
use espflash::connection::Connection;
use rustmeter_beacon::{buffer::BufferWriter, protocol::Request};

use std::{
    io::{ErrorKind, Read, Write},
    thread,
    time::Duration,
};

use crate::{
    commands::flash_and_monitor::ChipMonitoringTool,
    tracing::{CoreTracingData, TracingDecodeError},
};

pub struct SerialListener {
    tracing_bytes_recver: Receiver<Result<CoreTracingData, TracingDecodeError>>,
    req_sender: Sender<Request>,
}

impl SerialListener {
    pub fn new(espflash_conn: Connection) -> anyhow::Result<Self> {
        let (tracing_bytes_sender, tracing_bytes_recver) = crossbeam::channel::unbounded();
        let (req_sender, req_recver) = crossbeam::channel::unbounded();

        std::thread::spawn(move || {
            serial_reader_thread(espflash_conn, tracing_bytes_sender, req_recver)
        });

        Ok(Self {
            tracing_bytes_recver,
            req_sender,
        })
    }
}

impl ChipMonitoringTool for SerialListener {
    fn get_tracing_bytes_recver(&self) -> Receiver<Result<CoreTracingData, TracingDecodeError>> {
        self.tracing_bytes_recver.clone()
    }

    fn get_request_sender(&self) -> Sender<Request> {
        self.req_sender.clone()
    }
}

fn serial_reader_thread(
    espflash_conn: Connection,
    tracing_bytes_sender: Sender<Result<CoreTracingData, TracingDecodeError>>,
    req_recver: Receiver<Request>,
) {
    let mut serial_port = espflash_conn.into_serial();
    let mut buffer = [0u8; 4096];

    let mut decoding: Vec<u8> = Vec::with_capacity(buffer.len());
    let mut valid_in_stream = false; // check if we had previously valid frames in the stream

    loop {
        // Check for requests one at a time
        if let Ok(request) = req_recver.try_recv() {
            let mut writer = BufferWriter::new();
            request.write_bytes(&mut writer);

            if let Err(e) = serial_port.write_all(writer.as_slice()) {
                // Currently, we just log the error and continue
                println!("Warning: Failed to send request over serial port: {e}");
            }
        }

        // Try Read from serial port, else continue on timeout
        let read_count: usize = match serial_port.read(&mut buffer) {
            Ok(count) => count,
            Err(e) if e.kind() == ErrorKind::TimedOut => continue,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => {
                // Send error and continue
                let ch_closed = tracing_bytes_sender
                    .send(Err(TracingDecodeError::SerialPortError(e)))
                    .is_err();
                if ch_closed {
                    break;
                }

                thread::sleep(Duration::from_millis(100));
                continue;
            }
        };

        // add to decoding
        decoding.extend(&buffer[0..read_count]);

        // Try to decode (Frame starting with 0xFF, type-id, length of payload, payload, checksum)
        while let Some(frame_starts) = decoding.iter().position(|&b| b == 0xFF) {
            // Enforce minimum frame size (header)
            if decoding.len() < frame_starts + 4 {
                break;
            }

            // Read type id and length and check buffer size
            let type_id = decoding[frame_starts + 1];
            let length = decoding[frame_starts + 2] as usize;
            if decoding.len() < frame_starts + 4 + length {
                break;
            }

            // Calculate checksum
            let mut calculated_checksum: u8 = 0;
            for &b in &decoding[(frame_starts + 1)..(frame_starts + 3 + length)] {
                calculated_checksum ^= b;
            }

            // Verify checksum
            let received_checksum = decoding[frame_starts + 3 + length];
            if calculated_checksum != received_checksum {
                // Invalid checksum, discard this start byte and continue
                decoding.drain(0..(frame_starts + 1));

                if valid_in_stream {
                    let ch_closed = tracing_bytes_sender
                        .send(Err(TracingDecodeError::ChecksumMismatch))
                        .is_err();
                    if ch_closed {
                        break;
                    }
                }

                valid_in_stream = false;
                continue;
            }

            let payload = &decoding[(frame_starts + 3)..(frame_starts + 3 + length)];

            let ch_closed = match type_id {
                0xF0 => {
                    // tracing frame from core 0
                    valid_in_stream = true;
                    tracing_bytes_sender
                        .send(Ok(CoreTracingData::Core0(
                            payload.to_vec().into_boxed_slice(),
                        )))
                        .is_err()
                }
                0xF1 => {
                    // tracing frame from core 1
                    valid_in_stream = true;
                    tracing_bytes_sender
                        .send(Ok(CoreTracingData::Core1(
                            payload.to_vec().into_boxed_slice(),
                        )))
                        .is_err()
                }
                _ => {
                    // Unknown frame type, discard and continue
                    if valid_in_stream {
                        valid_in_stream = false;

                        tracing_bytes_sender
                            .send(Err(TracingDecodeError::InvalidFrameID(type_id)))
                            .is_err()
                    } else {
                        false
                    }
                }
            };

            if ch_closed {
                // Receiver has been closed, exit thread
                break;
            }

            // Remove processed frame from decoding buffer
            decoding.drain(0..(frame_starts + 4 + length));
        }
    }
}
