use crate::{
    buffer::{BufferReader, BufferWriter},
    protocol::{MonitorValuePayload, TypeDefinitionPayload},
};
use arbitrary_int::{traits::Integer, u3, u5};

pub type MonitorValueReaderFn =
    fn(monitor_id: u8, buffer: &mut BufferReader) -> Option<MonitorValuePayload>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventPayload {
    /// Embassy Task is ready to be polled (Waker called).
    /// CoreID is not included here because ISR can run on any core (mostly core 0).
    /// ExecutorID is not included here because the lookup of the short executor ID takes time and this event is called often (Task-Executor mapping is done via TaskNewEvent).
    EmbassyTaskReady { task_id: u16 },
    /// Embassy Task execution began (poll called).
    /// CoreID is included via Variant (Core0/Core1).
    /// ExecutorID is not included here because Task-Executor mapping is done via TaskNewEvent.
    EmbassyTaskExecBeginCore0 { task_id: u16 },
    /// Embassy Task execution began (poll called).
    /// CoreID is included via Variant (Core0/Core1).
    /// ExecutorID is not included here because Task-Executor mapping is done via TaskNewEvent
    EmbassyTaskExecBeginCore1 { task_id: u16 },
    /// Embassy Task execution ended (returned Poll::Ready or yielded Poll::Pending).
    /// CoreID is included via Variant (Core0/Core1).
    /// ExecutorID is included because it is shorter to transmit than TaskID and we know the executor from the TaskExecBegin event.
    EmbassyTaskExecEndCore0 { executor_id: u3 },
    /// Embassy Task execution ended (returned Poll::Ready or yielded Poll::Pending).
    /// CoreID is included via Variant (Core0/Core1).
    /// ExecutorID is included because it is shorter to transmit than TaskID and we know the executor from the TaskExecBegin event.
    EmbassyTaskExecEndCore1 { executor_id: u3 },
    /// Embassy Executor started polling tasks.
    /// ExecutorID is included because it is the only identifier for the executor.
    /// CoreID is not included here because executor than calls TaskExecBegin events that include the core ID (so this event can be taken out if not needed)
    EmbassyExecutorPollStart { executor_id: u3 },
    /// Embassy Executor is idle (no tasks to poll).
    /// ExecutorID is included because it is the only identifier for the executor.
    EmbassyExecutorIdle { executor_id: u3 },
    /// Function or Scope Monitor started
    /// CoreID is included via Variant (Core0/Core1).
    /// MonitorID identifies the monitor instance (was assigned via previous TypeDefinition event).
    MonitorStartCore0 { monitor_id: u8 },
    /// Function or Scope Monitor started
    /// CoreID is included via Variant (Core0/Core1).
    /// MonitorID identifies the monitor instance (was assigned via previous TypeDefinition event).
    MonitorStartCore1 { monitor_id: u8 },
    /// Function or Scope Monitor ended
    /// CoreID is included via Variant (Core0/Core1).
    /// MonitorID are not included here because they can be inferred from the corresponding MonitorStart event on the same core.
    MonitorEndCore0,
    /// Function or Scope Monitor ended
    /// CoreID is included via Variant (Core0/Core1).
    /// MonitorID are not included here because they can be inferred from the corresponding MonitorStart event
    MonitorEndCore1,
    /// Value Monitor reported a value
    /// ValueID identifies the monitor instance (was assigned via previous TypeDefinition event).
    /// Value is the reported value payload.
    /// CoreID is not relevant for value monitors and thus not included.
    MonitorValue {
        value_id: u8,
        value: MonitorValuePayload,
    },
    /// Type Definition Event
    TypeDefinition(TypeDefinitionPayload),
}

impl EventPayload {
    pub const fn event_id(&self) -> u5 {
        let id = match self {
            EventPayload::EmbassyTaskReady { .. } => 0,
            EventPayload::EmbassyTaskExecBeginCore0 { .. } => 1,
            EventPayload::EmbassyTaskExecBeginCore1 { .. } => 2,
            EventPayload::EmbassyTaskExecEndCore0 { .. } => 3,
            EventPayload::EmbassyTaskExecEndCore1 { .. } => 4,
            EventPayload::EmbassyExecutorPollStart { .. } => 5,
            EventPayload::EmbassyExecutorIdle { .. } => 6,
            EventPayload::MonitorStartCore0 { .. } => 7,
            EventPayload::MonitorStartCore1 { .. } => 8,
            EventPayload::MonitorEndCore0 => 9,
            EventPayload::MonitorEndCore1 => 10,
            EventPayload::MonitorValue { .. } => 11,
            EventPayload::TypeDefinition(_) => 12,
        };

        u5::new(id)
    }

    pub const fn get_executor_id(&self) -> Option<u3> {
        match self {
            EventPayload::EmbassyTaskExecEndCore0 { executor_id, .. } => Some(*executor_id),
            EventPayload::EmbassyTaskExecEndCore1 { executor_id, .. } => Some(*executor_id),
            EventPayload::EmbassyExecutorPollStart { executor_id, .. } => Some(*executor_id),
            EventPayload::EmbassyExecutorIdle { executor_id, .. } => Some(*executor_id),
            _ => None,
        }
    }

    pub(crate) fn write_bytes(&self, writer: &mut BufferWriter) {
        // Write the event ID (5 bits) and executor short ID (3 bits) as a single byte
        let executor_short_id = self.get_executor_id().map_or(u8::ZERO, |id| id.as_u8());
        let event_type = u8::from(self.event_id()) << 3 | executor_short_id;
        writer.write_byte(event_type);

        // Write event-specific data
        match self {
            EventPayload::EmbassyTaskReady { task_id } => {
                writer.write_bytes(&task_id.to_le_bytes());
            }
            EventPayload::EmbassyTaskExecBeginCore0 { task_id } => {
                writer.write_bytes(&task_id.to_le_bytes());
            }
            EventPayload::EmbassyTaskExecBeginCore1 { task_id } => {
                writer.write_bytes(&task_id.to_le_bytes());
            }
            EventPayload::EmbassyTaskExecEndCore0 { executor_id: _ } => {}
            EventPayload::EmbassyTaskExecEndCore1 { executor_id: _ } => {}
            EventPayload::EmbassyExecutorPollStart { executor_id: _ } => {}
            EventPayload::EmbassyExecutorIdle { executor_id: _ } => {}
            EventPayload::MonitorStartCore0 { monitor_id } => {
                writer.write_byte(*monitor_id);
            }
            EventPayload::MonitorStartCore1 { monitor_id } => {
                writer.write_byte(*monitor_id);
            }
            EventPayload::MonitorEndCore0 => {}
            EventPayload::MonitorEndCore1 => {}
            EventPayload::MonitorValue { value_id, value } => {
                writer.write_byte(*value_id);
                value.write_bytes(writer);
            }
            EventPayload::TypeDefinition(def) => {
                def.write_bytes(writer);
            }
        }
    }

    #[cfg(feature = "std")]
    /// Reads an EventPayload from the provided buffer based on the given type ID. Params:
    /// - event_type: The combined event type byte containing event ID and executor short ID.
    /// - buffer: The buffer reader to read additional event data from.
    /// - monitor_value_reader: A function to read MonitorValuePayloads, since they require additional context (e.q. Value Type of the monitor).
    pub(crate) fn from_bytes(
        event_type: u8,
        buffer: &mut BufferReader,
        monitor_value_reader: MonitorValueReaderFn,
    ) -> Option<EventPayload> {
        let event_id = u5::new(event_type >> 3);
        let _executor_short_id = u3::new(event_type & 0x07);

        match event_id.as_u8() {
            // EmbassyTaskReady
            0 => {
                let mut data = [0u8; 2];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Some(EventPayload::EmbassyTaskReady {
                    task_id: u16::from_le_bytes(data),
                })
            }
            // EmbassyTaskExecBeginCore0
            1 => {
                let mut data = [0u8; 2];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Some(EventPayload::EmbassyTaskExecBeginCore0 {
                    task_id: u16::from_le_bytes(data),
                })
            }
            // EmbassyTaskExecBeginCore
            2 => {
                let mut data = [0u8; 2];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Some(EventPayload::EmbassyTaskExecBeginCore1 {
                    task_id: u16::from_le_bytes(data),
                })
            }
            // EmbassyTaskExecEndCore0
            3 => Some(EventPayload::EmbassyTaskExecEndCore0 {
                executor_id: _executor_short_id,
            }),
            // EmbassyTaskExecEndCore1
            4 => Some(EventPayload::EmbassyTaskExecEndCore1 {
                executor_id: _executor_short_id,
            }),
            // EmbassyExecutorPollStart
            5 => Some(EventPayload::EmbassyExecutorPollStart {
                executor_id: _executor_short_id,
            }),
            // EmbassyExecutorIdle
            6 => Some(EventPayload::EmbassyExecutorIdle {
                executor_id: _executor_short_id,
            }),
            // MonitorStartCore0
            7 => {
                let monitor_id = buffer.read_byte()?;
                Some(EventPayload::MonitorStartCore0 { monitor_id })
            }
            // MonitorStartCore1
            8 => {
                let monitor_id = buffer.read_byte()?;
                Some(EventPayload::MonitorStartCore1 { monitor_id })
            }
            // MonitorEndCore0
            9 => Some(EventPayload::MonitorEndCore0),
            // MonitorEndCore1
            10 => Some(EventPayload::MonitorEndCore1),
            // MonitorValue
            11 => {
                let value_id = buffer.read_byte()?;
                let value = monitor_value_reader(value_id, buffer)?;
                Some(EventPayload::MonitorValue { value_id, value })
            }
            // TypeDefinition
            12 => {
                let typedef_it = buffer.read_byte()?;
                let def = TypeDefinitionPayload::from_bytes(typedef_it, buffer)?;
                Some(EventPayload::TypeDefinition(def))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use super::*;
    use crate::{
        buffer::{BufferReader, BufferWriter},
        protocol::{MonitorValuePayload, monitor_value_payload::MonitorValueType},
    };

    #[test]
    fn test_event_payload_write_and_read() {
        let events = vec![
            EventPayload::EmbassyTaskReady { task_id: 42 },
            EventPayload::EmbassyTaskExecBeginCore0 { task_id: 43 },
            EventPayload::EmbassyTaskExecBeginCore1 { task_id: 44 },
            EventPayload::EmbassyTaskExecEndCore0 {
                executor_id: u3::new(1),
            },
            EventPayload::EmbassyTaskExecEndCore1 {
                executor_id: u3::new(2),
            },
            EventPayload::EmbassyExecutorPollStart {
                executor_id: u3::new(3),
            },
            EventPayload::EmbassyExecutorIdle {
                executor_id: u3::new(4),
            },
            EventPayload::MonitorStartCore0 { monitor_id: 5 },
            EventPayload::MonitorStartCore1 { monitor_id: 6 },
            EventPayload::MonitorEndCore0,
            EventPayload::MonitorEndCore1,
            EventPayload::MonitorValue {
                value_id: 7,
                value: MonitorValuePayload::U32(123456),
            },
            EventPayload::TypeDefinition(TypeDefinitionPayload::ScopeMonitor {
                monitor_id: 8,
                name: "test_scope".to_string(),
            }),
        ];

        // create a closure to read MonitorValuePayloads for testing
        let monitor_value_reader = |monitor_id: u8, buffer: &mut BufferReader| {
            assert_eq!(monitor_id, 7); // we only test with monitor_id 7 here
            MonitorValuePayload::from_bytes(u32::ZERO.get_monitor_value_type_id(), buffer)
        };

        for event in events {
            // Write the event to bytes
            let mut writer = BufferWriter::new();
            event.write_bytes(&mut writer);
            let bytes = writer.as_slice();

            // Read the event back from bytes
            let mut reader = BufferReader::new(bytes);
            let read_event = EventPayload::from_bytes(
                reader.read_byte().unwrap(),
                &mut reader,
                monitor_value_reader,
            )
            .unwrap();

            assert_eq!(event, read_event);
        }
    }
}
