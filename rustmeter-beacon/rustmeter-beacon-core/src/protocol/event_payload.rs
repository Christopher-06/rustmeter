use crate::{
    buffer::{BufferReader, BufferWriter},
    protocol::{MonitorValuePayload, TypeDefinitionPayload},
};
use arbitrary_int::{traits::Integer, u3, u5};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventPayload {
    /// Embassy Task is ready to be polled (Waker called).
    /// ExecutorID will also be included
    EmbassyTaskReady { task_id: u16, executor_id: u3 },
    /// Embassy Task execution began (poll called).
    /// ExecutorID will also be included
    EmbassyTaskExecBegin { task_id: u16, executor_id: u3 },
    /// Embassy Task execution ended (returned Poll::Ready or yielded Poll::Pending).
    /// ExecutorID is included because it is shorter to transmit than TaskID and we know the executor from the TaskExecBegin event.
    EmbassyTaskExecEnd { executor_id: u3 },
    /// Embassy Executor started polling tasks.
    /// ExecutorID is included because it is the only identifier for the executor.
    EmbassyExecutorPollStart { executor_id: u3 },
    /// Embassy Executor is idle (no tasks to poll).
    /// ExecutorID is included because it is the only identifier for the executor.
    EmbassyExecutorIdle { executor_id: u3 },
    /// Function or Scope Monitor started
    /// MonitorID identifies the monitor instance (was assigned via previous TypeDefinition event).
    MonitorStart { monitor_id: u8 },
    /// Function or Scope Monitor ended
    /// MonitorID are not included here because they can be inferred from the corresponding MonitorStart event
    MonitorEnd,
    /// Value Monitor reported a value
    /// ValueID identifies the monitor instance (was assigned via previous TypeDefinition event).
    /// Value is the reported value payload.
    MonitorValue {
        value_id: u8,
        value: MonitorValuePayload,
    },
    /// Type Definition Event
    TypeDefinition(TypeDefinitionPayload),
    /// Data Loss Event because of buffer full situation
    DataLossEvent { dropped_events: u32 },
    DefmtData {
        len: u8,
        #[cfg(not(feature = "std"))]
        data: *const u8,
        #[cfg(feature = "std")]
        data: Vec<u8>,
    },
}

impl EventPayload {
    pub const fn event_id(&self) -> u5 {
        use crate::protocol::raw_writers::event_ids::*;
        let id = match self {
            EventPayload::EmbassyTaskReady { .. } => EMBASSY_TASK_READY,
            EventPayload::EmbassyTaskExecBegin { .. } => EMBASSY_TASK_EXEC_BEGIN,
            EventPayload::EmbassyTaskExecEnd { .. } => EMBASSY_TASK_EXEC_END,
            EventPayload::EmbassyExecutorPollStart { .. } => EMBASSY_EXECUTOR_POLL_START,
            EventPayload::EmbassyExecutorIdle { .. } => EMBASSY_EXECUTOR_IDLE,
            EventPayload::MonitorStart { .. } => MONITOR_START,
            EventPayload::MonitorEnd => MONITOR_END,
            EventPayload::MonitorValue { .. } => MONITOR_VALUE,
            EventPayload::TypeDefinition(_) => TYPE_DEFINITION,
            EventPayload::DataLossEvent { .. } => DATA_LOSS_EVENT,
            EventPayload::DefmtData { .. } => DEFMT_DATA_EVENT,
        };

        u5::new(id)
    }

    pub const fn get_executor_id(&self) -> Option<u3> {
        match self {
            EventPayload::EmbassyTaskReady { executor_id, .. } => Some(*executor_id),
            EventPayload::EmbassyTaskExecBegin { executor_id, .. } => Some(*executor_id),
            EventPayload::EmbassyTaskExecEnd { executor_id, .. } => Some(*executor_id),
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
            EventPayload::EmbassyTaskReady {
                task_id,
                executor_id: _,
            } => {
                writer.write_bytes(&task_id.to_le_bytes());
            }
            EventPayload::EmbassyTaskExecBegin {
                task_id,
                executor_id: _,
            } => {
                writer.write_bytes(&task_id.to_le_bytes());
            }
            EventPayload::EmbassyTaskExecEnd { executor_id: _ } => {}
            EventPayload::EmbassyExecutorPollStart { executor_id: _ } => {}
            EventPayload::EmbassyExecutorIdle { executor_id: _ } => {}
            EventPayload::MonitorStart { monitor_id } => {
                writer.write_byte(*monitor_id);
            }
            EventPayload::MonitorEnd => {}
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
            EventPayload::DefmtData { data, len } => {
                writer.write_byte(*len);
                #[cfg(not(feature = "std"))]
                unsafe {
                    writer.write_bytes(core::slice::from_raw_parts(*data, *len as usize));
                }
                #[cfg(feature = "std")]
                {
                    writer.write_bytes(&data[..*len as usize]);
                }
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

        use crate::protocol::raw_writers::event_ids::*;
        match event_id.as_u8() {
            // EmbassyTaskReady
            EMBASSY_TASK_READY => {
                let mut data = [0u8; 2];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Some(EventPayload::EmbassyTaskReady {
                    task_id: u16::from_le_bytes(data),
                    executor_id: _executor_short_id,
                })
            }
            // EmbassyTaskExecBegin
            EMBASSY_TASK_EXEC_BEGIN => {
                let mut data = [0u8; 2];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Some(EventPayload::EmbassyTaskExecBegin {
                    task_id: u16::from_le_bytes(data),
                    executor_id: _executor_short_id,
                })
            }
            // EmbassyTaskExecEnd
            EMBASSY_TASK_EXEC_END => Some(EventPayload::EmbassyTaskExecEnd {
                executor_id: _executor_short_id,
            }),
            // EmbassyExecutorPollStart
            EMBASSY_EXECUTOR_POLL_START => Some(EventPayload::EmbassyExecutorPollStart {
                executor_id: _executor_short_id,
            }),
            // EmbassyExecutorIdle
            EMBASSY_EXECUTOR_IDLE => Some(EventPayload::EmbassyExecutorIdle {
                executor_id: _executor_short_id,
            }),
            // MonitorStart
            MONITOR_START => {
                let monitor_id = buffer.read_byte()?;
                Some(EventPayload::MonitorStart { monitor_id })
            }
            // MonitorEnd
            MONITOR_END => Some(EventPayload::MonitorEnd),
            // MonitorValue
            MONITOR_VALUE => {
                let value_id = buffer.read_byte()?;
                let type_id = monitor_type_fn(value_id)?;
                let value = MonitorValuePayload::from_bytes(type_id, buffer)?;

                Some(EventPayload::MonitorValue { value_id, value })
            }
            // TypeDefinition
            TYPE_DEFINITION => {
                let typedef_it = buffer.read_byte()?;
                let def = TypeDefinitionPayload::from_bytes(typedef_it, buffer)?;
                Some(EventPayload::TypeDefinition(def))
            }
            // DataLossEvent
            DATA_LOSS_EVENT => {
                let mut data = [0u8; 4];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }

                Some(EventPayload::DataLossEvent {
                    dropped_events: u32::from_le_bytes(data),
                })
            }
            // DefmtDataEvent
            DEFMT_DATA_EVENT => {
                #[cfg(not(feature = "std"))]
                {
                    panic!("DefmtDataEvent decoding requires the 'std' feature to be enabled.");
                }
                #[cfg(feature = "std")]
                {
                    let len = buffer.read_byte()?;

                    // Read data
                    let mut data = vec![0u8; len as usize];
                    for byte in data.iter_mut() {
                        *byte = buffer.read_byte()?;
                    }
                    Some(EventPayload::DefmtData { len, data })
                }
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
            EventPayload::EmbassyTaskReady {
                task_id: 42,
                executor_id: u3::new(5),
            },
            EventPayload::EmbassyTaskExecBeginCore0 {
                task_id: 43,
                executor_id: u3::new(5),
            },
            EventPayload::EmbassyTaskExecBeginCore1 {
                task_id: 44,
                executor_id: u3::new(6),
            },
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
