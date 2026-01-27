use crossbeam::channel::{Receiver, Sender};
use espflash::connection::Connection;
use rustmeter_beacon_core::{buffer::BufferWriter, protocol::Request};

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
    let mut next_seq_id: [Option<u8>; 2] = [None, None];

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

            // Read seq id, core id and length
            let core_id = ((decoding[frame_starts + 1] >> 7) & 0x01) as usize;
            let seq_id = decoding[frame_starts + 1] & 0x7F;
            let length = decoding[frame_starts + 2] as usize;
            if decoding.len() < frame_starts + 4 + length {
                break;
            }

            // Check sequence id
            assert!(
                core_id < 2,
                "Core ID should be one bit. This should never happen."
            );
            let as_expected =
                next_seq_id[core_id].is_none_or(|expected_seq_id| seq_id == expected_seq_id);
            if !as_expected {
                // Sequence ID mismatch, discard this start byte and continue
                decoding.drain(0..(frame_starts + 1));

                if valid_in_stream {
                    let ch_closed = tracing_bytes_sender
                        .send(Err(TracingDecodeError::SequenceIdMismatch {
                            expected: next_seq_id[core_id].unwrap(),
                            received: seq_id,
                            core_id: core_id as u8,
                        }))
                        .is_err();
                    if ch_closed {
                        break;
                    }
                }

                // clear valid state
                valid_in_stream = false;
                next_seq_id = [None, None];
                continue;
            }
            next_seq_id[core_id] = Some((seq_id + 1) % 128);

            // Calculate checksum
            let mut calculated_checksum: u8 = 0;
            for &b in &decoding[frame_starts..(frame_starts + 3 + length)] {
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
                next_seq_id = [None, None];
                continue;
            }

            // Prepare tracing bytes
            let payload = &decoding[(frame_starts + 3)..(frame_starts + 3 + length)];
            let tracing_bytes = match core_id {
                0 => CoreTracingData::Core0(payload.to_vec().into_boxed_slice()),
                1 => CoreTracingData::Core1(payload.to_vec().into_boxed_slice()),
                // This should never happen because of one bit core id.
                _ => unreachable!("Invalid core id parsed."),
            };

            // Send tracing bytes
            valid_in_stream = true;
            let ch_closed = tracing_bytes_sender.send(Ok(tracing_bytes)).is_err();
            if ch_closed {
                // Receiver has been closed, exit thread
                break;
            }

            // Remove processed frame from decoding buffer
            decoding.drain(0..(frame_starts + 4 + length));
        }
    }
}
