use crossbeam::channel::{Receiver, Sender};
use espflash::connection::Connection;

use std::io::{ErrorKind, Read};

use crate::ChipMonitoringTool;

pub struct SerialListener {
    defmt_bytes_recver: Receiver<Box<[u8]>>,
    tracing_bytes_recver: Receiver<Box<[u8]>>,
    error_recver: Receiver<anyhow::Error>,
}

impl SerialListener {
    pub fn new(espflash_conn: Connection) -> anyhow::Result<Self> {
        let (defmt_bytes_sender, defmt_bytes_recver) = crossbeam::channel::unbounded();
        let (tracing_bytes_sender, tracing_bytes_recver) = crossbeam::channel::unbounded();
        let (error_sender, error_recver) = crossbeam::channel::unbounded();

        std::thread::spawn(move || {
            serial_reader_thread(
                espflash_conn,
                defmt_bytes_sender,
                tracing_bytes_sender,
                error_sender,
            )
        });

        Ok(Self {
            defmt_bytes_recver,
            tracing_bytes_recver,
            error_recver,
        })
    }
}

impl ChipMonitoringTool for SerialListener {
    fn get_defmt_bytes_recver(&self) -> Receiver<Box<[u8]>> {
        self.defmt_bytes_recver.clone()
    }

    fn get_tracing_bytes_recver(&self) -> Receiver<Box<[u8]>> {
        self.tracing_bytes_recver.clone()
    }

    fn get_error_recver(&self) -> Receiver<anyhow::Error> {
        self.error_recver.clone()
    }
}

fn serial_reader_thread(
    espflash_conn: Connection,
    defmt_bytes_sender: Sender<Box<[u8]>>,
    tracing_bytes_sender: Sender<Box<[u8]>>,
    error_sender: Sender<anyhow::Error>,
) {
    let mut serial_port = espflash_conn.into_serial();
    let mut buffer = [0u8; 4096];

    let mut decoding: Vec<u8> = Vec::new();

    loop {
        // Try Read from serial port
        let read_count: usize = match serial_port.read(&mut buffer) {
            Ok(count) => count,
            Err(e) if e.kind() == ErrorKind::TimedOut => 0,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => {
                let _ =
                    error_sender.send(anyhow::Error::new(e).context("Failed to read serial_port"));
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
            let received_checksum = decoding[frame_starts + 3 + length];
            if calculated_checksum != received_checksum {
                // Invalid checksum, discard this start byte and continue
                decoding.drain(0..(frame_starts + 1));
                let _ = error_sender.send(anyhow::anyhow!("Invalid checksum in serial frame"));
                continue;
            }

            let paylaod = &decoding[(frame_starts + 3)..(frame_starts + 3 + length)];

            match type_id {
                1 => {
                    // tracing frame
                    let _ = tracing_bytes_sender.send(paylaod.to_vec().into_boxed_slice());
                }
                2 => {
                    // defmt frame
                    let _ = defmt_bytes_sender.send(paylaod.to_vec().into_boxed_slice());
                }
                _ => {
                    println!("Unknown frame type id: {}", type_id);
                }
            }

            // Remove processed frame from decoding buffer
            decoding.drain(0..(frame_starts + 4 + length));
        }
    }
}
