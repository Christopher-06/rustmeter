use arbitrary_int::{traits::Integer, u3};

use crate::buffer::{BufferReader, BufferWriter};

/// Type Definition Event Payloads
#[derive(Debug, Clone, PartialEq, Eq)]
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
    ScopeMonitor {
        monitor_id: u8,
        #[cfg(not(feature = "std"))]
        name: &'static str,
        #[cfg(feature = "std")]
        name: String,
    },
    /// New Value Monitor defined
    /// ValueID identifies the monitor instance in future events.
    /// TypeID identifies the type of the value being monitored (see MonitorValueType).
    /// Name is a null-terminated string representing the name of the value (max. 20 Characters).
    ValueMonitor {
        value_id: u8,
        type_id: u8,
        #[cfg(not(feature = "std"))]
        name: &'static str,
        #[cfg(feature = "std")]
        name: String,
    },
}

impl TypeDefinitionPayload {
    pub const fn type_id(&self) -> u8 {
        match self {
            TypeDefinitionPayload::EmbassyTaskCreated { .. } => 0,
            TypeDefinitionPayload::EmbassyTaskEnded { .. } => 1,
            TypeDefinitionPayload::FunctionMonitor { .. } => 2,
            TypeDefinitionPayload::ScopeMonitor { .. } => 3,
            TypeDefinitionPayload::ValueMonitor { .. } => 4,
        }
    }

    pub(crate) fn write_bytes(&self, writer: &mut BufferWriter) {
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

    #[cfg(feature = "std")]
    pub(crate) fn from_bytes(typedef_id: u8, buffer: &mut BufferReader) -> Option<Self> {
        match typedef_id {
            // EmbassyTaskCreated
            0 => {
                // Read full TaskID
                let mut task_id_bytes = [0u8; 4];
                for byte in task_id_bytes.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                let task_id = u32::from_le_bytes(task_id_bytes);

                // Read full ExecutorIDLong
                let mut executor_id_long_bytes = [0u8; 4];
                for byte in executor_id_long_bytes.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                let executor_id_long = u32::from_le_bytes(executor_id_long_bytes);

                // Read ExecutorIDShort
                let executor_id_short = u3::new(buffer.read_byte()?);

                Some(TypeDefinitionPayload::EmbassyTaskCreated {
                    task_id,
                    executor_id_long,
                    executor_id_short,
                })
            }
            // EmbassyTaskEnded
            1 => {
                // Read full TaskID
                let mut task_id_bytes = [0u8; 4];
                for byte in task_id_bytes.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                let task_id = u32::from_le_bytes(task_id_bytes);

                // Read full ExecutorIDLong
                let mut executor_id_long_bytes = [0u8; 4];
                for byte in executor_id_long_bytes.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                let executor_id_long = u32::from_le_bytes(executor_id_long_bytes);

                // Read ExecutorIDShort
                let executor_id_short = u3::new(buffer.read_byte()?);

                Some(TypeDefinitionPayload::EmbassyTaskEnded {
                    task_id,
                    executor_id_long,
                    executor_id_short,
                })
            }
            // FunctionMonitor
            2 => {
                let monitor_id = buffer.read_byte()?;

                // Read FnAddress
                let mut fn_address_bytes = [0u8; 4];
                for byte in fn_address_bytes.iter_mut() {
                    *byte = buffer.read_byte()?;
                }
                let fn_address = u32::from_le_bytes(fn_address_bytes);

                Some(TypeDefinitionPayload::FunctionMonitor {
                    monitor_id,
                    fn_address,
                })
            }
            // ScopeMonitor
            3 => {
                // Read MonitorID
                let monitor_id = buffer.read_byte()?;

                // Read null-terminated string
                let mut name_bytes = Vec::new();
                loop {
                    let byte = buffer.read_byte()?;
                    if byte == 0 {
                        break;
                    }
                    name_bytes.push(byte);
                }
                let name = core::str::from_utf8(&name_bytes).ok()?.to_string();

                Some(TypeDefinitionPayload::ScopeMonitor { monitor_id, name })
            }
            // ValueMonitor
            4 => {
                // Read ValueID
                let value_id = buffer.read_byte()?;

                // Read TypeID
                let type_id = buffer.read_byte()?;

                // Read null-terminated string
                let mut name_bytes = Vec::new();
                loop {
                    let byte = buffer.read_byte()?;
                    if byte == 0 {
                        break;
                    }
                    name_bytes.push(byte);
                }
                let name = core::str::from_utf8(&name_bytes).ok()?.to_string();

                Some(TypeDefinitionPayload::ValueMonitor {
                    value_id,
                    type_id,
                    name,
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
    use crate::buffer::{BufferReader, BufferWriter};

    #[test]
    fn test_type_definition_read_and_write() {
        let typedefs = vec![
            TypeDefinitionPayload::EmbassyTaskCreated {
                task_id: 0x12345678,
                executor_id_long: 0x9ABCDEF0,
                executor_id_short: u3::new(5),
            },
            TypeDefinitionPayload::EmbassyTaskEnded {
                task_id: 0x87654321,
                executor_id_long: 0x0FEDCBA9,
                executor_id_short: u3::new(3),
            },
            TypeDefinitionPayload::FunctionMonitor {
                monitor_id: 42,
                fn_address: 0xDEADBEEF,
            },
            TypeDefinitionPayload::ScopeMonitor {
                monitor_id: 7,
                name: "TestScope".to_string(),
            },
            TypeDefinitionPayload::ValueMonitor {
                value_id: 13,
                type_id: 2,
                name: "TestValue".to_string(),
            },
        ];

        for typedef in typedefs {
            // Write typedef to bytes
            let mut writer = BufferWriter::new();
            typedef.write_bytes(&mut writer);
            let bytes = writer.as_slice();

            // Read typedef from bytes
            let mut reader = BufferReader::new(bytes);
            let typedef_id = reader.read_byte().unwrap();
            let parsed_typedef =
                TypeDefinitionPayload::from_bytes(typedef_id, &mut reader).unwrap();

            assert_eq!(typedef, parsed_typedef);
        }
    }
}
