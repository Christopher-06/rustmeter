use crate::{buffer::BufferWriter, protocol::EventPayload, time_delta::TimeDelta};

unsafe extern "Rust" {
    /// Low-level function to write tracing data. Implemented in the target crate.
    fn write_tracing_data(data: &[u8]);
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
