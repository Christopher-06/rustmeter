use core::mem::MaybeUninit;

use crate::{protocol::EventPayload, time_delta::TimeDelta};

unsafe extern "Rust" {
    /// Low-level function to write tracing data. Implemented in the target crate.
    fn write_tracing_data(data: &[u8]);
}

/// Internal buffer writer for tracing events using a fixed-size buffer with uninitialized memory for efficiency
pub(crate) struct BufferWriter {
    buffer: [MaybeUninit<u8>; 32],
    position: usize,
}

impl BufferWriter {
    pub fn new() -> Self {
        BufferWriter {
            buffer: [MaybeUninit::uninit(); 32],
            position: 0,
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        self.buffer[self.position] = MaybeUninit::new(byte);
        self.position += 1;
    }

    /// Writes a slice of bytes into the buffer. Assumes there is enough space
    pub fn write_bytes(&mut self, data: &[u8]) {
        let len = data.len();
        self.buffer[self.position..self.position + len]
            .copy_from_slice(unsafe { core::mem::transmute::<&[u8], &[MaybeUninit<u8>]>(data) });
        self.position += len;
    }

    /// Returns the already written data as a slice
    pub fn as_slice(&self) -> &[u8] {
        &unsafe { core::mem::transmute::<&[MaybeUninit<u8>], &[u8]>(&self.buffer[..self.position]) }
    }
}

/// Serializes and writes a tracing event with timestamp to the tracing channel
pub fn write_tracing_event(event: EventPayload) {
    let timestamp = TimeDelta::from_now();

    // Write event data
    let mut buffer = BufferWriter::new();
    timestamp.write(&mut buffer);
    event.write_bytes(&mut buffer);

    // Send the data over RTT
    unsafe { write_tracing_data(buffer.as_slice()) };
}

pub struct BufferReader<'a> {
    buffer: &'a [u8],
    position: usize,
}

/// Simple buffer reader for reading bytes from a slice
impl<'a> BufferReader<'a> {
    pub fn new(buffer: &'a [u8]) -> Self {
        BufferReader { buffer, position: 0 }
    }

    /// Reads a single byte from the buffer. Returns None if end of buffer is reached.
    pub fn read_byte(&mut self) -> Option<u8> {
        if self.position >= self.buffer.len() {
            return None;
        }

        let byte = self.buffer[self.position];
        self.position += 1;
        Some(byte)
    }

    /// Reads a slice of bytes of the given length from the buffer. Returns None if not enough data is available.
    pub fn read_bytes(&mut self, length: usize) -> Option<&[u8]> {
        if self.position + length > self.buffer.len() {
            return None;
        }

        let bytes = &self.buffer[self.position..self.position + length];
        self.position += length;
        Some(bytes)
    }
}