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
    espflash::framing::FrameStream,
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
                if valid_in_stream {
                    valid_in_stream = false;

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

        // Try to decode frames
        decoding.extend(&buffer[0..read_count]);
        while let Some((frame_pos, decoded)) = try_decode_single(&decoding, &mut next_seq_id) {
            // handle decoded frame
            let ch_closed = match decoded {
                Ok((frame_len, tracing_data)) => {
                    // drain decoded frame from stream
                    decoding.drain(0..(frame_pos + frame_len));

                    valid_in_stream = true;
                    tracing_bytes_sender.send(Ok(tracing_data)).is_err()
                }
                Err(e) => {
                    // drain till frame start + 1
                    decoding.drain(0..(frame_pos + 1));

                    // send one error when we had valid frames before
                    if valid_in_stream {
                        valid_in_stream = false;

                        tracing_bytes_sender.send(Err(e)).is_err()
                    } else {
                        // send empty data to check if channel is closed
                        tracing_bytes_sender
                            .send(Ok(CoreTracingData::Core0(Box::new([]))))
                            .is_err()
                    }
                }
            };

            // end if channel closed
            if ch_closed {
                return;
            }
        }
    }
}

/// Try to decode a single frame from the given byte slice. Returns the position of the frame in the
/// slice and the decoded CoreTracingData with frame len, or a TracingDecodeError if decoding failed.
fn try_decode_single(
    buffer: &[u8],
    next_seq_id: &mut [Option<u8>],
) -> Option<(usize, Result<(usize, CoreTracingData), TracingDecodeError>)> {
    // Try to get first frame
    let stream = FrameStream::from_bytes(&buffer);
    let (frame_pos, frame) = match stream.get_first_frame_start(None) {
        Some((pos, frame)) => (pos, frame),
        None => return None,
    };

    // TODO: Handle incomplete frames when Panic Mode starts
    // TODO: Handle same seq id in Panic Mode??? Handle stream failure before Panic Mode!!!

    if !frame.is_complete() {
        return None;
    }

    // Check checksum
    if !frame.verify_checksum().unwrap() {
        return Some((frame_pos, Err(TracingDecodeError::ChecksumMismatch)));
    }

    // Check seq id
    let core_id = frame.core_id().unwrap() as usize;
    let seq_id = frame.seq_id().unwrap();
    if !next_seq_id[core_id].is_none_or(|expected| seq_id == expected) {
        return Some((
            frame_pos,
            Err(TracingDecodeError::SequenceIdMismatch {
                expected: next_seq_id[core_id].unwrap(),
                received: seq_id,
                core_id: core_id as u8,
            }),
        ));
    }
    next_seq_id[core_id] = Some((seq_id + 1) % 128);

    // return tracing data
    let payload = frame.data().unwrap();
    let core_mapper = match core_id {
        0 => CoreTracingData::Core0,
        1 => CoreTracingData::Core1,
        // This should never happen because of one bit core id.
        _ => unreachable!("Invalid core id parsed."),
    };
    Some((
        frame_pos,
        Ok((
            frame.len().unwrap(),
            core_mapper(payload.to_vec().into_boxed_slice()),
        )),
    ))
}
