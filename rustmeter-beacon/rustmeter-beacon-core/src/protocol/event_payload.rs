use crate::{
    buffer::{BufferReader, BufferWriter},
    protocol::{MonitorValuePayload, TypeDefinitionPayload},
};
use arbitrary_int::{traits::Integer, u3, u5};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventPayload {
    /// Embassy Task is ready to be polled (Waker called).
    /// CoreID is not included here because ISR can run on any core (mostly core 0).
    /// ExecutorID will also be included
    EmbassyTaskReady { task_id: u16, executor_id: u3 },
    /// Embassy Task execution began (poll called).
    /// CoreID is included via Variant (Core0/Core1).
    /// ExecutorID will also be included
    EmbassyTaskExecBeginCore0 { task_id: u16, executor_id: u3 },
    /// Embassy Task execution began (poll called).
    /// CoreID is included via Variant (Core0/Core1).
    /// ExecutorID will also be included
    EmbassyTaskExecBeginCore1 { task_id: u16, executor_id: u3 },
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
    /// Data Loss Event because of buffer full situation
    DataLossEvent { dropped_events: u32 },
}

impl EventPayload {
    pub const fn event_id(&self) -> u5 {
        use crate::protocol::raw_writers::event_ids::*;
        let id = match self {
            EventPayload::EmbassyTaskReady { .. } => EMBASSY_TASK_READY,
            EventPayload::EmbassyTaskExecBeginCore0 { .. } => EMBASSY_TASK_EXEC_BEGIN_CORE0,
            EventPayload::EmbassyTaskExecBeginCore1 { .. } => EMBASSY_TASK_EXEC_BEGIN_CORE1,
            EventPayload::EmbassyTaskExecEndCore0 { .. } => EMBASSY_TASK_EXEC_END_CORE0,
            EventPayload::EmbassyTaskExecEndCore1 { .. } => EMBASSY_TASK_EXEC_END_CORE1,
            EventPayload::EmbassyExecutorPollStart { .. } => EMBASSY_EXECUTOR_POLL_START,
            EventPayload::EmbassyExecutorIdle { .. } => EMBASSY_EXECUTOR_IDLE,
            EventPayload::MonitorStartCore0 { .. } => MONITOR_START_CORE0,
            EventPayload::MonitorStartCore1 { .. } => MONITOR_START_CORE1,
            EventPayload::MonitorEndCore0 => MONITOR_END_CORE0,
            EventPayload::MonitorEndCore1 => MONITOR_END_CORE1,
            EventPayload::MonitorValue { .. } => MONITOR_VALUE,
            EventPayload::TypeDefinition(_) => TYPE_DEFINITION,
            EventPayload::DataLossEvent { .. } => DATA_LOSS_EVENT,
        };

        u5::new(id)
    }

    pub const fn get_executor_id(&self) -> Option<u3> {
        match self {
            EventPayload::EmbassyTaskReady { executor_id, .. } => Some(*executor_id),
            EventPayload::EmbassyTaskExecBeginCore0 { executor_id, .. } => Some(*executor_id),
            EventPayload::EmbassyTaskExecBeginCore1 { executor_id, .. } => Some(*executor_id),
            EventPayload::EmbassyTaskExecEndCore0 { executor_id, .. } => Some(*executor_id),
            EventPayload::EmbassyTaskExecEndCore1 { executor_id, .. } => Some(*executor_id),
            EventPayload::EmbassyExecutorPollStart { executor_id, .. } => Some(*executor_id),
            EventPayload::EmbassyExecutorIdle { executor_id, .. } => Some(*executor_id),
            _ => None,
        }
    }

    pub fn write_bytes(&self, writer: &mut BufferWriter) {
        // Write the event ID (5 bits) and executor short ID (3 bits) as a single byte
        let executor_short_id = self.get_executor_id().map_or(u8::ZERO, |id| id.as_u8());
        let event_type = u8::from(self.event_id()) << 3 | executor_short_id;
        writer.write_byte(event_type);

        // Write event-specific data
        match self {
            EventPayload::EmbassyTaskReady { task_id, executor_id : _ } => {
                writer.write_bytes(&task_id.to_le_bytes());
            }
            EventPayload::EmbassyTaskExecBeginCore0 { task_id, executor_id: _ } => {
                writer.write_bytes(&task_id.to_le_bytes());
            }
            EventPayload::EmbassyTaskExecBeginCore1 { task_id, executor_id: _ } => {
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
            EventPayload::DataLossEvent { dropped_events } => {
                writer.write_bytes(&dropped_events.to_le_bytes());
            }
        }
    }

    /// Reads an EventPayload from the provided buffer based on the given type ID. Params:
    /// - event_type: The combined event type byte containing event ID and executor short ID.
    /// - buffer: The buffer reader to read additional event data from.
    /// - monitor_value_reader: A function to map monitor IDs to their corresponding ValueTypes.
    pub fn from_bytes<F>(
        event_type: u8,
        buffer: &mut BufferReader,
        monitor_type_fn: &F,
    ) -> Option<EventPayload>
    where
        F: Fn(u8) -> Option<u8>,
    {
        let event_id = u5::new(event_type >> 3);
        let _executor_short_id = u3::new(event_type & 0x07);

        match event_id.as_u8() {
            // EmbassyTaskReady
            1 => {
                let mut data = [0u8; 2];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Some(EventPayload::EmbassyTaskReady {
                    task_id: u16::from_le_bytes(data),
                    executor_id: _executor_short_id,
                })
            }
            // EmbassyTaskExecBeginCore0
            2 => {
                let mut data = [0u8; 2];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Some(EventPayload::EmbassyTaskExecBeginCore0 {
                    task_id: u16::from_le_bytes(data),
                    executor_id: _executor_short_id,
                })
            }
            // EmbassyTaskExecBeginCore
            3 => {
                let mut data = [0u8; 2];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Some(EventPayload::EmbassyTaskExecBeginCore1 {
                    task_id: u16::from_le_bytes(data),
                    executor_id: _executor_short_id,
                })
            }
            // EmbassyTaskExecEndCore0
            4 => Some(EventPayload::EmbassyTaskExecEndCore0 {
                executor_id: _executor_short_id,
            }),
            // EmbassyTaskExecEndCore1
            5 => Some(EventPayload::EmbassyTaskExecEndCore1 {
                executor_id: _executor_short_id,
            }),
            // EmbassyExecutorPollStart
            6 => Some(EventPayload::EmbassyExecutorPollStart {
                executor_id: _executor_short_id,
            }),
            // EmbassyExecutorIdle
            7 => Some(EventPayload::EmbassyExecutorIdle {
                executor_id: _executor_short_id,
            }),
            // MonitorStartCore0
            8 => {
                let monitor_id = buffer.read_byte()?;
                Some(EventPayload::MonitorStartCore0 { monitor_id })
            }
            // MonitorStartCore1
            9 => {
                let monitor_id = buffer.read_byte()?;
                Some(EventPayload::MonitorStartCore1 { monitor_id })
            }
            // MonitorEndCore0
            10 => Some(EventPayload::MonitorEndCore0),
            // MonitorEndCore1
            11 => Some(EventPayload::MonitorEndCore1),
            // MonitorValue
            12 => {
                let value_id = buffer.read_byte()?;
                let type_id = monitor_type_fn(value_id)?;
                let value = MonitorValuePayload::from_bytes(type_id, buffer)?;

                Some(EventPayload::MonitorValue { value_id, value })
            }
            // TypeDefinition
            13 => {
                let typedef_it = buffer.read_byte()?;
                let def = TypeDefinitionPayload::from_bytes(typedef_it, buffer)?;
                Some(EventPayload::TypeDefinition(def))
            }
            // DataLossEvent
            14 => {
                let mut data = [0u8; 4];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }

                Some(EventPayload::DataLossEvent {
                    dropped_events: u32::from_le_bytes(data),
                })
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
            EventPayload::EmbassyTaskReady { task_id: 42, executor_id: u3::new(5) },
            EventPayload::EmbassyTaskExecBeginCore0 { task_id: 43, executor_id: u3::new(5) },
            EventPayload::EmbassyTaskExecBeginCore1 { task_id: 44, executor_id: u3::new(6) },
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
            EventPayload::DataLossEvent { dropped_events: 10 },
        ];

        // create a closure to read MonitorValuePayloads for testing
        let monitor_value_reader = |monitor_id: u8| {
            assert_eq!(monitor_id, 7); // we only test with monitor_id 7 here
            Some(u32::ZERO.get_monitor_value_type_id())
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
                &monitor_value_reader,
            )
            .unwrap();

            assert_eq!(event, read_event);
        }
    }
}
