#[cfg(feature = "std")]
use crate::protocol::MonitorValueReaderFn;
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
    timestamp.write_bytes(&mut buffer);
    event.write_bytes(&mut buffer);

    // Send the data over RTT
    unsafe { write_tracing_data(buffer.as_slice()) };
}

#[cfg(feature = "std")]
pub fn read_tracing_event(
    data: &[u8],
    monitor_value_reader: MonitorValueReaderFn,
) -> Option<(TimeDelta, EventPayload)> {
    let mut reader = crate::buffer::BufferReader::new(data);

    let timestamp = TimeDelta::read_bytes(&mut reader)?;
    let event_type = reader.read_byte()?;
    let event = EventPayload::from_bytes(event_type, &mut reader, monitor_value_reader)?;

    Some((timestamp, event))
}

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use arbitrary_int::u3;

    use super::*;
    use crate::{
        buffer::BufferReader,
        protocol::{MonitorValuePayload, MonitorValueType, TypeDefinitionPayload},
        time_delta::TimeDelta,
    };
    use arbitrary_int::traits::Integer;
    use std::sync::atomic::AtomicU32;

    // Mock implementation of get_tracing_time_us for testing
    #[unsafe(no_mangle)]
    fn get_tracing_time_us() -> u32 {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        COUNTER.fetch_add(1000, std::sync::atomic::Ordering::SeqCst)
    }

    #[test]
    fn test_tracing_event_write_and_read() {
        let events = vec![
            EventPayload::EmbassyTaskReady { task_id: 1234 },
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
                value: MonitorValuePayload::U16(65535),
            },
        ];

        let monitor_value_reader = |monitor_id: u8, buffer: &mut BufferReader| {
            assert_eq!(monitor_id, 1);
            MonitorValuePayload::from_bytes(u16::ZERO.get_monitor_value_type_id(), buffer)
        };

        for event in events {
            // Write event
            let mut buffer = BufferWriter::new();
            let timestamp = TimeDelta::from_now();
            timestamp.write_bytes(&mut buffer);
            event.write_bytes(&mut buffer);
            let data = buffer.as_slice();

            // Read event
            let (read_timestamp, read_event) = read_tracing_event(data, monitor_value_reader)
                .expect("Failed to read tracing event");

            assert_eq!(timestamp, read_timestamp);
            assert_eq!(event, read_event);
        }
    }
}
