use arbitrary_int::{traits::Integer, u3, u5};

// TODO: Add Event Documentation

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

    pub(crate) fn write_bytes(&self, writer: &mut crate::tracing::BufferWriter) {
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
                let mut data_buffer = [0u8; 8]; // Max size needed for u64/i64
                let data_size = value.data_bytes(&mut data_buffer);
                writer.write_bytes(&data_buffer[..data_size]);
            }
            EventPayload::TypeDefinition(def) => {
                def.write_bytes(writer);
            }
        }
    }
}

/// Type Definition Event Payloads
pub enum TypeDefinitionPayload {
    /// New Embassy Task created.
    /// TaskID is the full task ID used in TaskReady events. (Can be compressed on host side to gather shorter taskid)
    /// ExecutorIDLong is the full executor ID used to identify the executor instance.
    /// ExecutorIDShort is the short executor ID used in events to identify the executor instance
    EmbassyTaskCreated {
        task_id: u32,
        executor_id_long: u32,
        executor_id_short: u3,
    },
    /// Embassy Task ended
    /// TaskID is the full task ID used in TaskReady events. (Can be compressed on host side to gather shorter taskid)
    /// ExecutorIDLong is the full executor ID used to identify the executor instance.
    /// ExecutorIDShort is the short executor ID used in events to identify the executor instance
    /// This event indicates that the task will not be scheduled again.
    EmbassyTaskEnded {
        task_id: u32,
        executor_id_long: u32,
        executor_id_short: u3,
    },
    /// New Function Monitor defined
    /// MonitorID identifies the monitor instance in future events.
    /// FnAddress is the function address being monitored.
    FunctionMonitor { monitor_id: u8, fn_address: u32 },
    /// New Scope Monitor defined
    /// MonitorID identifies the monitor instance in future events.
    /// Name is a null-terminated string representing the name of the scope (max. 20 Characters).
    ScopeMonitor { monitor_id: u8, name: &'static str },
    /// New Value Monitor defined
    /// ValueID identifies the monitor instance in future events.
    /// TypeID identifies the type of the value being monitored (see MonitorValueType).
    /// Name is a null-terminated string representing the name of the value (max. 20 Characters).
    ValueMonitor {
        value_id: u8,
        type_id: u8,
        name: &'static str,
    },
}

impl TypeDefinitionPayload {
    pub const fn type_id(&self) -> u8 {
        match self {
            TypeDefinitionPayload::EmbassyTaskCreated { .. } => 0,
            TypeDefinitionPayload::EmbassyTaskEnded { .. } => 1,
            TypeDefinitionPayload::FunctionMonitor { .. } => 3,
            TypeDefinitionPayload::ScopeMonitor { .. } => 4,
            TypeDefinitionPayload::ValueMonitor { .. } => 5,
        }
    }

    pub(crate) fn write_bytes(&self, writer: &mut crate::tracing::BufferWriter) {
        // Write the type definition ID as first byte
        writer.write_byte(self.type_id());

        // Write type definition specific data
        match self {
            TypeDefinitionPayload::EmbassyTaskCreated {
                task_id,
                executor_id_long,
                executor_id_short,
            } => {
                writer.write_bytes(&task_id.to_le_bytes()); // send full task ID for mapping
                writer.write_bytes(&executor_id_long.to_le_bytes());
                writer.write_byte(executor_id_short.as_u8());
            }
            TypeDefinitionPayload::EmbassyTaskEnded {
                task_id,
                executor_id_long,
                executor_id_short,
            } => {
                writer.write_bytes(&task_id.to_le_bytes()); // send full task ID for mapping
                writer.write_bytes(&executor_id_long.to_le_bytes());
                writer.write_byte(executor_id_short.as_u8());
            }
            TypeDefinitionPayload::FunctionMonitor {
                monitor_id,
                fn_address,
            } => {
                writer.write_byte(*monitor_id);
                writer.write_bytes(&fn_address.to_le_bytes());
            }
            TypeDefinitionPayload::ScopeMonitor { monitor_id, name } => {
                writer.write_byte(*monitor_id);
                writer.write_bytes(name.as_bytes());
                writer.write_byte(0); // Null-terminated string
            }
            TypeDefinitionPayload::ValueMonitor {
                value_id,
                type_id,
                name,
            } => {
                writer.write_byte(*value_id);
                writer.write_byte(*type_id);
                writer.write_bytes(name.as_bytes());
                writer.write_byte(0); // Null-terminated string
            }
        }
    }
}

/// Payloads for Monitor Value Events
pub enum MonitorValuePayload {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
}

impl MonitorValuePayload {
    pub fn type_id(&self) -> u8 {
        // cannot be u3 because one byte must be used for length encoding
        match self {
            MonitorValuePayload::U8(x) => x.get_monitor_value_type_id(),
            MonitorValuePayload::U16(x) => x.get_monitor_value_type_id(),
            MonitorValuePayload::U32(x) => x.get_monitor_value_type_id(),
            MonitorValuePayload::U64(x) => x.get_monitor_value_type_id(),
            MonitorValuePayload::I8(x) => x.get_monitor_value_type_id(),
            MonitorValuePayload::I16(x) => x.get_monitor_value_type_id(),
            MonitorValuePayload::I32(x) => x.get_monitor_value_type_id(),
            MonitorValuePayload::I64(x) => x.get_monitor_value_type_id(),
        }
    }

    /// Write the payload data into the provided buffer.
    /// Returns the number of data bytes written into the provided buffer. Assumes the buffer is large enough.
    pub fn data_bytes(&self, buffer: &mut [u8]) -> usize {
        match self {
            MonitorValuePayload::U8(v) => {
                buffer[0] = *v;
                1
            }
            MonitorValuePayload::U16(v) => {
                buffer[0..2].copy_from_slice(&v.to_le_bytes());
                2
            }
            MonitorValuePayload::U32(v) => {
                buffer[0..4].copy_from_slice(&v.to_le_bytes());
                4
            }
            MonitorValuePayload::U64(v) => {
                buffer[0..8].copy_from_slice(&v.to_le_bytes());
                8
            }
            MonitorValuePayload::I8(v) => {
                buffer[0] = *v as u8;
                1
            }
            MonitorValuePayload::I16(v) => {
                buffer[0..2].copy_from_slice(&v.to_le_bytes());
                2
            }
            MonitorValuePayload::I32(v) => {
                buffer[0..4].copy_from_slice(&v.to_le_bytes());
                4
            }
            MonitorValuePayload::I64(v) => {
                buffer[0..8].copy_from_slice(&v.to_le_bytes());
                8
            }
        }
    }
}

pub trait MonitorValueType {
    fn to_payload(&self) -> MonitorValuePayload;
    fn get_monitor_value_type_id(&self) -> u8;
}

impl MonitorValueType for u8 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::U8(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        0
    }
}

impl MonitorValueType for u16 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::U16(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        1
    }
}

impl MonitorValueType for u32 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::U32(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        2
    }
}

impl MonitorValueType for u64 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::U64(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        3
    }
}

impl MonitorValueType for i8 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::I8(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        4
    }
}

impl MonitorValueType for i16 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::I16(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        5
    }
}

impl MonitorValueType for i32 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::I32(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        6
    }
}

impl MonitorValueType for i64 {
    fn to_payload(&self) -> MonitorValuePayload {
        MonitorValuePayload::I64(*self)
    }

    fn get_monitor_value_type_id(&self) -> u8 {
        7
    }
}
