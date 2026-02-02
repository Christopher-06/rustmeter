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
    espflash::serial_decoder::SerialDecoder,
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

    let mut frame_decoder = SerialDecoder::new();
    let mut valid_connection = true;

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
                if valid_connection {
                    valid_connection = false;

                    let ch_closed = tracing_bytes_sender
                        .send(Err(TracingDecodeError::SerialPortError(e)))
                        .is_err();
                    if ch_closed {
                        break;
                    }
                }

                thread::sleep(Duration::from_millis(100));
                continue;
            }
        };
        frame_decoder.feed(&buffer[0..read_count]);

        // Try to decode frames
        loop {
            let data = match frame_decoder.try_decode() {
                Ok(None) => break, // no more frames
                Ok(Some(data)) => Ok(data),
                Err(e) => Err(e),
            };

            if tracing_bytes_sender.send(data).is_err() {
                return;
            }
        }
    }
}
