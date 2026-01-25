use arbitrary_int::{traits::Integer, u3};

use crate::{buffer::{BufferReader, BufferWriter}, tracing::ReadTracingError};

/// Type Definition Event Payloads (as tagged enum)
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
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
    /// Name is a null-terminated string representing the name of the value (max. 20 Characters).
    ValueMonitor {
        value_id: u8,
        #[cfg(not(feature = "std"))]
        name: &'static str,
        #[cfg(feature = "std")]
        name: String,
    },
    GlobalClockConfiguration {
        system_frequency_hz: u32,
        tick_divider: u16,
    },
    /// Core Clock Reference Event. Syncing CPU ticks to system timer time and core ID.
    /// Allows to calibrate different cores against each other and the system time.
    /// This also helps to match cpu ticks with Log Lines
    CoreClockReference {
        core_id: u8,
        systimer_us: u64,
        cpu_ticks: u32,
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
            TypeDefinitionPayload::GlobalClockConfiguration { .. } => 5,
            TypeDefinitionPayload::CoreClockReference { .. } => 6,
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
            TypeDefinitionPayload::ValueMonitor { value_id, name } => {
                writer.write_byte(*value_id);
                writer.write_bytes(name.as_bytes());
                writer.write_byte(0); // Null-terminated string
            }
            TypeDefinitionPayload::GlobalClockConfiguration {
                system_frequency_hz,
                tick_divider,
            } => {
                writer.write_bytes(&system_frequency_hz.to_le_bytes());
                writer.write_bytes(&tick_divider.to_le_bytes());
            }
            TypeDefinitionPayload::CoreClockReference {
                core_id,
                systimer_us,
                cpu_ticks,
            } => {
                writer.write_byte(*core_id);
                writer.write_bytes(&systimer_us.to_le_bytes());
                writer.write_bytes(&cpu_ticks.to_le_bytes());
            }
        }
    }

    pub(crate) fn from_bytes(typedef_id: u8, buffer: &mut BufferReader) -> Result<Self, ReadTracingError> {
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

                Ok(TypeDefinitionPayload::EmbassyTaskCreated {
                    task_id,
                    executor_id_long,
                    executor_id_short,
                })
            }
            // EmbassyTaskEnded
            1 => {
                let task_id = buffer.read_u32()?;
                let executor_id_long = buffer.read_u32()?;
                let executor_id_short = u3::new(buffer.read_byte()?);

                Ok(TypeDefinitionPayload::EmbassyTaskEnded {
                    task_id,
                    executor_id_long,
                    executor_id_short,
                })
            }
            // FunctionMonitor
            2 => {
                let monitor_id = buffer.read_byte()?;
                let fn_address = buffer.read_u32()?;

                Ok(TypeDefinitionPayload::FunctionMonitor {
                    monitor_id,
                    fn_address,
                })
            }
            // ScopeMonitor
            3 => {
                #[cfg(not(feature = "std"))]
                todo!("Have string decoding working in no_std first");

                #[cfg(feature = "std")]
                {
                    // Read MonitorID
                    let monitor_id = buffer.read_byte()?;

                    // Read null-terminated string
                    let mut name_bytes = std::vec::Vec::new();
                    loop {
                        let byte = buffer.read_byte()?;
                        if byte == 0 {
                            break;
                        }
                        name_bytes.push(byte);
                    }
                    let name = core::str::from_utf8(&name_bytes)?.to_string();

                    Ok(TypeDefinitionPayload::ScopeMonitor { monitor_id, name })
                }
            }
            // ValueMonitor
            4 => {
                #[cfg(not(feature = "std"))]
                todo!("Have string decoding working in no_std first");

                #[cfg(feature = "std")]
                {
                    // Read ValueID
                    let value_id = buffer.read_byte()?;

                    // Read null-terminated string
                    let mut name_bytes = Vec::new();
                    loop {
                        let byte = buffer.read_byte()?;
                        if byte == 0 {
                            break;
                        }
                        name_bytes.push(byte);
                    }
                    let name = core::str::from_utf8(&name_bytes)?.to_string();

                    Ok(TypeDefinitionPayload::ValueMonitor { value_id, name })
                }
            }
            // GlobalClockConfiguration
            5 => {
                let system_frequency_hz = buffer.read_u32()?;
                let tick_divider = buffer.read_u16()?;

                Ok(TypeDefinitionPayload::GlobalClockConfiguration {
                    system_frequency_hz,
                    tick_divider,
                })
            }
            // CoreClockReference
            6 => {
                let core_id = buffer.read_byte()?;
                let systimer_us = buffer.read_u64()?;
                let cpu_ticks = buffer.read_u32()?;

                Ok(TypeDefinitionPayload::CoreClockReference {
                    core_id,
                    systimer_us,
                    cpu_ticks,
                })
            }
            _ => Err(ReadTracingError::InvalidTypedefID),
        }
    }
}

#[cfg(test)]
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
                name: "TestValue".to_string(),
            },
            TypeDefinitionPayload::GlobalClockConfiguration {
                system_frequency_hz: 48_000_000,
                tick_divider: 64,
            },
            TypeDefinitionPayload::CoreClockReference {
                core_id: 1,
                systimer_us: 1_000_000,
                cpu_ticks: 123456,
            },
        ];

        for typedef in typedefs {
            // Write the event to bytes
            let mut writer = BufferWriter::new();
            typedef.write_bytes(&mut writer);
            let bytes = writer.as_slice();

            // Read the event back from bytes
            let mut reader = BufferReader::new(bytes);
            let read_typedef =
                TypeDefinitionPayload::from_bytes(reader.read_byte().unwrap(), &mut reader)
                    .unwrap();

            assert_eq!(typedef, read_typedef);
        }
    }
}
