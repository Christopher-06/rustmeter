use critical_section::Mutex;

use crate::{
    buffer::{BufferReader, BufferWriter},
    protocol::EventPayload,
    time_delta::TimeDelta,
};

unsafe extern "Rust" {
    /// Low-level function to write tracing data. Implemented in the target crate.
    pub fn write_tracing_data(data: &[u8]);
}

static TRACE_WRITING: Mutex<()> = Mutex::new(());

/// Serializes and writes a tracing event with timestamp to the tracing channel
pub fn write_tracing_event(event: EventPayload) {
    // Ensure only one tracing event is written at a time
    critical_section::with(|cs| {
        let _lock = TRACE_WRITING.borrow(cs);

        let timestamp = TimeDelta::from_now();

        // Write event data
        let mut buffer = BufferWriter::new();
        event.write_bytes(&mut buffer);
        timestamp.write_bytes(&mut buffer);

        // Send the data over RTT
        unsafe { write_tracing_data(buffer.as_slice()) };
    });
}

pub fn read_tracing_event(buffer: &mut BufferReader) -> Option<(TimeDelta, EventPayload)> {
    let event_type = buffer.read_byte()?;
    let event = EventPayload::from_bytes(event_type, buffer)?;
    let timestamp = TimeDelta::read_bytes(buffer)?;

    Some((timestamp, event))
}

#[cfg(test)]
mod tests {
    use arbitrary_int::u3;

    use super::*;
    use crate::{
        mocks::test_mocks::{mock_time_provider, mock_trace_writer, with_mocks},
        protocol::TypeDefinitionPayload,
    };

    #[test]
    fn test_tracing_event_write_and_read() {
        let events = vec![
            EventPayload::EmbassyTaskReady {
                task_id: 1234,
                executor_id: u3::new(2),
            },
            EventPayload::EmbassyExecutorPollStart {
                executor_id: u3::new(5),
            },
            EventPayload::TypeDefinition(TypeDefinitionPayload::FunctionMonitor {
                monitor_id: 42,
                fn_address: 0xDEADBEEF,
            }),
            EventPayload::TypeDefinition(TypeDefinitionPayload::ScopeMonitor {
                monitor_id: 7,
                name: "TestScope".to_string(),
            }),
            EventPayload::MonitorValue {
                value_id: 1,
                value: 456i16.into(),
            },
        ];

        for event in events {
            with_mocks(
                mock_trace_writer(event.clone()),
                mock_time_provider,
                || 0,
                || {
                    write_tracing_event(event);
                },
            );
        }
    }
}
