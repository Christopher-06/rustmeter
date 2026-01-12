use crate::{
    buffer::{BufferReader, BufferWriter},
    protocol::{MonitorValuePayload, TypeDefinitionPayload},
    tracing::ReadTracingError,
};
use arbitrary_int::{traits::Integer, u3, u5};

#[derive(Debug, Clone, PartialEq)]
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

    /// Returns the executor ID if the event is related to an embassy executor
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

    /// Returns the MonitorValuePayload type ID if the event is a MonitorValue event
    pub const fn get_monitor_value_type_id(&self) -> Option<u3> {
        match self {
            EventPayload::MonitorValue { value, .. } => Some(value.type_id()),
            _ => None,
        }
    }

    /// Returns the sub ID (executor ID or MonitorValue type ID) if applicable
    pub const fn get_sub_id(&self) -> Option<u3> {
        // Check for executor ID
        if let Some(executor_id) = self.get_executor_id() {
            return Some(executor_id);
        }
        // Check for MonitorValue type ID
        if let Some(type_id) = self.get_monitor_value_type_id() {
            return Some(type_id);
        }

        None
    }

    pub fn write_bytes(&self, writer: &mut BufferWriter) {
        // Write the event ID (5 bits) and sub event id (3 bits) as a single byte
        let sub_id = self.get_sub_id().unwrap_or(u3::new(0));
        let event_type = u8::from(self.event_id()) << 3 | sub_id.as_u8();
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
    pub fn from_bytes(
        event_type: u8,
        buffer: &mut BufferReader,
    ) -> Result<EventPayload, ReadTracingError> {
        let event_id = u5::new(event_type >> 3);
        let sub_id = u3::new(event_type & 0x07);

        use crate::protocol::raw_writers::event_ids::*;
        match event_id.as_u8() {
            // EmbassyTaskReady
            EMBASSY_TASK_READY => {
                let mut data = [0u8; 2];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Ok(EventPayload::EmbassyTaskReady {
                    task_id: u16::from_le_bytes(data),
                    executor_id: sub_id,
                })
            }
            // EmbassyTaskExecBegin
            EMBASSY_TASK_EXEC_BEGIN => {
                let mut data = [0u8; 2];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                Ok(EventPayload::EmbassyTaskExecBegin {
                    task_id: u16::from_le_bytes(data),
                    executor_id: sub_id,
                })
            }
            // EmbassyTaskExecEnd
            EMBASSY_TASK_EXEC_END => Ok(EventPayload::EmbassyTaskExecEnd {
                executor_id: sub_id,
            }),
            // EmbassyExecutorPollStart
            EMBASSY_EXECUTOR_POLL_START => Ok(EventPayload::EmbassyExecutorPollStart {
                executor_id: sub_id,
            }),
            // EmbassyExecutorIdle
            EMBASSY_EXECUTOR_IDLE => Ok(EventPayload::EmbassyExecutorIdle {
                executor_id: sub_id,
            }),
            // MonitorStart
            MONITOR_START => {
                let monitor_id = buffer.read_byte()?;
                Ok(EventPayload::MonitorStart { monitor_id })
            }
            // MonitorEnd
            MONITOR_END => Ok(EventPayload::MonitorEnd),
            // MonitorValue
            MONITOR_VALUE => {
                let value_id = buffer.read_byte()?;
                let value = MonitorValuePayload::from_bytes(sub_id, buffer)?;

                Ok(EventPayload::MonitorValue { value_id, value })
            }
            // TypeDefinition
            TYPE_DEFINITION => {
                let typedef_it = buffer.read_byte()?;
                let def = TypeDefinitionPayload::from_bytes(typedef_it, buffer)?;
                Ok(EventPayload::TypeDefinition(def))
            }
            // DataLossEvent
            DATA_LOSS_EVENT => {
                let mut data = [0u8; 4];
                for byte in data.iter_mut() {
                    *byte = buffer.read_byte()?;
                }

                Ok(EventPayload::DataLossEvent {
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
                    Ok(EventPayload::DefmtData { len, data })
                }
            }
            _ => return Err(ReadTracingError::InvalidEventType),
        }
    }
}

#[cfg(test)]
#[cfg(feature = "std")]
mod tests {
    use super::*;
    use crate::buffer::{BufferReader, BufferWriter};

    #[test]
    fn test_event_payload_write_and_read() {
        let events = vec![
            EventPayload::EmbassyTaskReady {
                task_id: 42,
                executor_id: u3::new(5),
            },
            EventPayload::EmbassyTaskExecBegin {
                task_id: 44,
                executor_id: u3::new(6),
            },
            EventPayload::EmbassyTaskExecEnd {
                executor_id: u3::new(1),
            },
            EventPayload::EmbassyExecutorPollStart {
                executor_id: u3::new(3),
            },
            EventPayload::EmbassyExecutorIdle {
                executor_id: u3::new(4),
            },
            EventPayload::MonitorStart { monitor_id: 5 },
            EventPayload::MonitorEnd,
            EventPayload::MonitorValue {
                value_id: 7,
                value: 456u16.into(),
            },
            EventPayload::TypeDefinition(TypeDefinitionPayload::ScopeMonitor {
                monitor_id: 8,
                name: "test_scope".to_string(),
            }),
            EventPayload::DataLossEvent { dropped_events: 10 },
        ];

        for event in events {
            // Write the event to bytes
            let mut writer = BufferWriter::new();
            event.write_bytes(&mut writer);
            let bytes = writer.as_slice();

            // Read the event back from bytes
            let mut reader = BufferReader::new(bytes);
            let read_event =
                EventPayload::from_bytes(reader.read_byte().unwrap(), &mut reader).unwrap();

            assert_eq!(event, read_event);
        }
    }
}
