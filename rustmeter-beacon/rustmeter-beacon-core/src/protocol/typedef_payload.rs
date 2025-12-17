use arbitrary_int::{traits::Integer, u3};

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
