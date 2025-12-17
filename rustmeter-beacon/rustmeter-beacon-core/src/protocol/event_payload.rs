use arbitrary_int::{traits::Integer, u3, u5};
use crate::protocol::{TypeDefinitionPayload, MonitorValuePayload};

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
                value.write_bytes(writer);
            }
            EventPayload::TypeDefinition(def) => {
                def.write_bytes(writer);
            }
        }
    }
}
